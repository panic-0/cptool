use super::data::{
    GenerateOptions, data_generation_status, format_duration, generate_data_with_options,
    wait_for_generation_status,
};
use super::judge::run_configured_checker_on_files;
use super::problem::{load_problem, normalize_work_dir, resolve_path};
use super::schema::{DEFAULT_OUTPUT_LIMIT_BYTES, Problem, ProgramInfo, StressPlanExpectation};
use super::temp_suffix;
use std::collections::{HashMap, HashSet};
use std::path::Path;

mod codes;
mod markdown_sample;
mod package_audit;
mod report;
mod yaml_shape;

use markdown_sample::{check_statement_sample_output, find_sample_bundle};
use package_audit::{check_package_text_audit, check_report_stress_plan_classification};
pub use report::{CheckIssue, CheckOptions, CheckReport, CheckSeverity};
use yaml_shape::check_unknown_yaml_fields;

#[cfg(test)]
use markdown_sample::{
    markdown_sample_output_blocks, normalize_output_block, sample_answer_from_data_dir,
};
#[cfg(test)]
use std::path::PathBuf;

pub fn check_problem_package(work_dir: &Path) -> CheckReport {
    check_problem_package_with_options(work_dir, CheckOptions::default())
}

pub fn check_problem_package_with_options(work_dir: &Path, options: CheckOptions) -> CheckReport {
    let work_dir = normalize_work_dir(work_dir).unwrap_or_else(|_| work_dir.to_path_buf());
    let mut report = CheckReport::new(work_dir.clone());

    check_required_files(&mut report, &work_dir);
    check_unknown_yaml_fields(&mut report, &work_dir);
    check_package_text_audit(&mut report, &work_dir);

    let problem = match load_problem(&work_dir) {
        Ok(problem) => problem,
        Err(err) => {
            report.error(
                codes::PROBLEM_YAML_INVALID,
                format!("problem.yaml could not be loaded or validated: {err:#}"),
                Some(work_dir.join("problem.yaml")),
            );
            check_statement_sample_output(&mut report, &work_dir, None, None);
            return report;
        }
    };

    check_program_paths(&mut report, &work_dir, &problem);
    check_problem_structure(&mut report, &work_dir, &problem);
    check_validator_declaration(&mut report, &work_dir, &problem);
    check_stress_plans(&mut report, &work_dir, &problem);
    check_report_stress_plan_classification(&mut report, &work_dir, &problem);
    let data_dir = work_dir.join("data");
    let generation_status = if let Some(timeout) = options.generation_lock_timeout {
        wait_for_generation_status(&data_dir, timeout)
    } else {
        data_generation_status(&data_dir)
    };
    if let Some(status) = generation_status {
        let message = if let Some(timeout) = options.generation_lock_timeout {
            format!(
                "data generation is still in progress after waiting {}; skipped data consistency checks to avoid reading partial output; retry after current generation finishes or prewarm the selector serially",
                format_duration(timeout)
            )
        } else {
            "data generation is in progress; skipped data consistency checks to avoid reading partial output".to_string()
        };
        report.lock_error(
            codes::DATA_GENERATION_IN_PROGRESS,
            message,
            Some(status.marker_path),
        );
        return report;
    }

    check_generated_data(&mut report, &work_dir, &problem);

    let generated_sample_answer = check_sample_generation(&mut report, &work_dir, &problem);
    check_statement_sample_output(
        &mut report,
        &work_dir,
        Some(&problem),
        generated_sample_answer.as_deref(),
    );

    report
}

fn check_required_files(report: &mut CheckReport, work_dir: &Path) {
    for relative in [
        "problem.yaml",
        "statement.md",
        "editorial.md",
        "src/std.cpp",
    ] {
        let path = work_dir.join(relative);
        if !path.is_file() {
            report.error(
                codes::REQUIRED_FILE_MISSING,
                format!("required file `{relative}` is missing"),
                Some(path),
            );
        }
    }
}

fn check_program_paths(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    for (name, program) in &problem.programs {
        let raw_path = match &program.info {
            ProgramInfo::Command(program) => &program.path,
            ProgramInfo::Cpp(program) => &program.path,
            ProgramInfo::Python(program) => &program.path,
        };
        let path = resolve_path(work_dir, raw_path);
        if !path.is_file() {
            report.error(
                codes::PROGRAM_PATH_MISSING,
                format!("program `{name}` path does not exist"),
                Some(path),
            );
        }
    }
}

fn check_problem_structure(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    let yaml_path = work_dir.join("problem.yaml");
    if problem.test.tasks.is_empty() {
        report.error_at(
            codes::TEST_TASKS_EMPTY,
            "`test.tasks` must contain at least one task",
            Some(yaml_path.clone()),
            "test.tasks",
        );
    }

    for (bundle_name, bundle) in &problem.test.bundles {
        if bundle.cases.is_empty() {
            report.error_at(
                codes::BUNDLE_EMPTY,
                format!("bundle `{bundle_name}` has no cases"),
                Some(yaml_path.clone()),
                format!("test.bundles.{bundle_name}.cases"),
            );
        }
    }

    let mut used_bundles = HashSet::new();
    let mut task_index_by_name = HashMap::new();
    for (index, task) in problem.test.tasks.iter().enumerate() {
        task_index_by_name.insert(task.name.as_str(), index);
        if task.bundles.is_empty() {
            report.error_at(
                codes::TASK_HAS_NO_BUNDLES,
                format!("task `{}` has no bundles", task.name),
                Some(yaml_path.clone()),
                format!("test.tasks[{index}].bundles"),
            );
        }
        used_bundles.extend(task.bundles.iter().cloned());
    }

    let total_score = problem
        .test
        .tasks
        .iter()
        .map(|task| task.score)
        .sum::<f64>();
    if !problem.test.tasks.is_empty() && (total_score - 100.0).abs() > 1e-6 {
        report.warning_at(
            codes::TASK_SCORE_TOTAL_NOT_100,
            format!("task scores sum to {total_score}, expected 100.0"),
            Some(yaml_path.clone()),
            "test.tasks",
        );
    }

    for bundle_name in problem.test.bundles.keys() {
        if !used_bundles.contains(bundle_name) {
            report.warning_at(
                codes::BUNDLE_UNCOVERED_BY_TASK,
                format!("bundle `{bundle_name}` is not referenced by any task"),
                Some(yaml_path.clone()),
                format!("test.bundles.{bundle_name}"),
            );
        }
    }

    if let Some(cycle) = task_dependency_cycle(problem, &task_index_by_name) {
        report.error_at(
            codes::TASK_DEPENDENCY_CYCLE,
            format!("task dependencies contain a cycle: {}", cycle.join(" -> ")),
            Some(yaml_path),
            "test.tasks",
        );
    }
}

fn check_validator_declaration(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    if problem.validator_name.is_some() {
        return;
    }
    if problem
        .validator_omitted_reason
        .as_deref()
        .is_some_and(|reason| !reason.trim().is_empty())
    {
        return;
    }

    report.warning(
        codes::VALIDATOR_MISSING,
        "`validator` is not declared; add one or set `validator_omitted_reason`",
        Some(work_dir.join("problem.yaml")),
    );
}

fn check_generated_data(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    check_expected_data_files(report, work_dir, problem);
    check_stale_data_files(report, work_dir, problem);
    check_empty_answers(report, work_dir, problem);
}

fn check_expected_data_files(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    let data_dir = work_dir.join("data");
    let existing_data_files = count_data_io_files(&data_dir);
    let missing_kind = if existing_data_files == 0 {
        "not_generated"
    } else {
        "missing"
    };
    let next_action = format!("cptool case gen -w {} --clean", work_dir.display());
    for (bundle_name, bundle) in &problem.test.bundles {
        for case_index in 0..bundle.cases.len() {
            for extension in ["in", "ans"] {
                let path = data_dir.join(format!("{bundle_name}-{case_index}.{extension}"));
                if !path.is_file() {
                    let message = if missing_kind == "not_generated" {
                        format!(
                            "generated data file `{bundle_name}-{case_index}.{extension}` is missing because no generated .in/.ans files are present"
                        )
                    } else {
                        format!(
                            "generated data file `{bundle_name}-{case_index}.{extension}` is missing from an existing generated data set"
                        )
                    };
                    report.action_error_at(
                        codes::GENERATED_DATA_MISSING,
                        message,
                        Some(path),
                        format!("test.bundles.{bundle_name}.cases[{case_index}]"),
                        missing_kind,
                        next_action.clone(),
                    );
                }
            }
        }
    }
}

fn check_stale_data_files(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    let data_dir = work_dir.join("data");
    let Ok(entries) = std::fs::read_dir(&data_dir) else {
        return;
    };
    let next_action = format!("cptool case gen -w {} --clean", work_dir.display());
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_data_io_file(&path) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some((bundle_name, case_index)) = parse_data_file_stem(stem) else {
            report.action_warning(
                codes::STALE_DATA_FILE,
                "data file does not match `<bundle>-<index>.in/.ans` naming",
                Some(path),
                "stale",
                next_action.clone(),
            );
            continue;
        };
        let Some(bundle) = problem.test.bundles.get(bundle_name) else {
            report.action_warning(
                codes::STALE_DATA_FILE,
                format!("data file references unknown bundle `{bundle_name}`"),
                Some(path),
                "stale",
                next_action.clone(),
            );
            continue;
        };
        if case_index >= bundle.cases.len() {
            report.action_warning(
                codes::STALE_DATA_FILE,
                format!("data file references missing case `{bundle_name}[{case_index}]`"),
                Some(path),
                "stale",
                next_action.clone(),
            );
        }
    }
}

fn check_empty_answers(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    let data_dir = work_dir.join("data");
    if !data_dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&data_dir) else {
        report.warning(
            codes::DATA_DIR_UNREADABLE,
            "data directory exists but could not be read",
            Some(data_dir),
        );
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("ans") {
            continue;
        }
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.len() == 0 && !problem.output.allow_empty => report.error(
                codes::EMPTY_ANSWER,
                ".ans file is empty but output.allow_empty is not declared",
                Some(path),
            ),
            Ok(_) => {}
            Err(err) => report.warning(
                codes::ANSWER_UNREADABLE,
                format!("could not inspect .ans file: {err}"),
                Some(path),
            ),
        }
    }
}

fn check_stress_plans(report: &mut CheckReport, work_dir: &Path, problem: &Problem) {
    let yaml_path = work_dir.join("problem.yaml");
    let plans = &problem.stress.plans;
    if plans.is_empty() {
        report.warning_at(
            codes::STRESS_PLANS_MISSING,
            "`stress.plans` is not declared",
            Some(yaml_path),
            "stress.plans",
        );
        return;
    }

    if !plans
        .iter()
        .any(|plan| plan.expect == StressPlanExpectation::Pass)
    {
        report.warning_at(
            codes::STRESS_POSITIVE_PLAN_MISSING,
            "`stress.plans` has no `expect: pass` plan",
            Some(work_dir.join("problem.yaml")),
            "stress.plans",
        );
    }

    for (index, plan) in plans.iter().enumerate() {
        let location = format!("stress.plans[{index}]");
        if plan.cases == 0 {
            report.error_at(
                codes::STRESS_PLAN_EMPTY,
                format!("stress plan `{}` has zero cases", plan.name),
                Some(work_dir.join("problem.yaml")),
                format!("{location}.cases"),
            );
        }
        if plan.against.len() < 2 {
            report.error_at(
                codes::STRESS_PLAN_AGAINST_TOO_FEW,
                format!(
                    "stress plan `{}` must compare at least two programs or sources",
                    plan.name
                ),
                Some(work_dir.join("problem.yaml")),
                format!("{location}.against"),
            );
        }
        for (field, value) in std::iter::once(("generator", plan.generator.as_str())).chain(
            plan.against
                .iter()
                .map(|target| ("against", target.as_str())),
        ) {
            if !stress_program_exists(work_dir, problem, value) {
                report.error_at(
                    codes::STRESS_PLAN_PROGRAM_MISSING,
                    format!(
                        "stress plan `{}` {field} `{value}` is neither a configured program nor an existing source file",
                        plan.name
                    ),
                    Some(work_dir.join("problem.yaml")),
                    location.clone(),
                );
            }
        }
    }
}

fn task_dependency_cycle<'a>(
    problem: &'a Problem,
    task_index_by_name: &HashMap<&'a str, usize>,
) -> Option<Vec<String>> {
    fn visit(
        index: usize,
        problem: &Problem,
        task_index_by_name: &HashMap<&str, usize>,
        state: &mut [u8],
        stack: &mut Vec<usize>,
    ) -> Option<Vec<String>> {
        if state[index] == 1 {
            let start = stack.iter().position(|&item| item == index).unwrap_or(0);
            let mut cycle = stack[start..]
                .iter()
                .map(|&item| problem.test.tasks[item].name.clone())
                .collect::<Vec<_>>();
            cycle.push(problem.test.tasks[index].name.clone());
            return Some(cycle);
        }
        if state[index] == 2 {
            return None;
        }

        state[index] = 1;
        stack.push(index);
        for dependency in &problem.test.tasks[index].dependencies {
            if let Some(&dependency_index) = task_index_by_name.get(dependency.as_str())
                && let Some(cycle) =
                    visit(dependency_index, problem, task_index_by_name, state, stack)
            {
                return Some(cycle);
            }
        }
        stack.pop();
        state[index] = 2;
        None
    }

    let mut state = vec![0u8; problem.test.tasks.len()];
    let mut stack = Vec::new();
    for index in 0..problem.test.tasks.len() {
        if let Some(cycle) = visit(index, problem, task_index_by_name, &mut state, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

fn stress_program_exists(work_dir: &Path, problem: &Problem, value: &str) -> bool {
    if problem.programs.contains_key(value) {
        return true;
    }
    let path = resolve_path(work_dir, Path::new(value));
    path.is_file()
        && matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("cpp" | "cc" | "cxx" | "py")
        )
}

fn is_data_io_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("in" | "ans")
    )
}

fn count_data_io_files(data_dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(data_dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.is_file() && is_data_io_file(&path)
        })
        .count()
}

fn parse_data_file_stem(stem: &str) -> Option<(&str, usize)> {
    let (bundle, index) = stem.rsplit_once('-')?;
    if bundle.is_empty() {
        return None;
    }
    Some((bundle, index.parse().ok()?))
}

fn check_sample_generation(
    report: &mut CheckReport,
    work_dir: &Path,
    problem: &Problem,
) -> Option<String> {
    let Some(sample_bundle) = find_sample_bundle(problem) else {
        report.warning(
            codes::SAMPLE_BUNDLE_MISSING,
            "no `sample` bundle is declared, so sample data generation was not checked",
            Some(work_dir.join("problem.yaml")),
        );
        return None;
    };
    if problem
        .test
        .bundles
        .get(sample_bundle)
        .is_none_or(|bundle| bundle.cases.is_empty())
    {
        report.error(
            codes::SAMPLE_BUNDLE_EMPTY,
            format!("sample bundle `{sample_bundle}` has no cases"),
            Some(work_dir.join("problem.yaml")),
        );
        return None;
    }

    let output_dir = std::env::temp_dir().join(format!("cptool-check-{}", temp_suffix()));
    let result = generate_data_with_options(GenerateOptions {
        work_dir: work_dir.to_path_buf(),
        bundle: Some(sample_bundle.to_string()),
        selector: None,
        output_dir: Some(output_dir.clone()),
        output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
        clean: false,
        generation_lock_timeout: None,
    });
    let generated = match result {
        Ok(generated) => generated,
        Err(err) => {
            report.error(
                codes::SAMPLE_GENERATION_FAILED,
                format!("sample data generation failed for bundle `{sample_bundle}`: {err:#}"),
                Some(work_dir.join("problem.yaml")),
            );
            let _ = std::fs::remove_dir_all(&output_dir);
            return None;
        }
    };

    for path in &generated {
        if path.extension().and_then(|ext| ext.to_str()) == Some("ans")
            && std::fs::metadata(path).is_ok_and(|metadata| metadata.len() == 0)
            && !problem.output.allow_empty
        {
            report.error(
                codes::EMPTY_ANSWER,
                "generated sample .ans is empty but output.allow_empty is not declared",
                None,
            );
        }
    }

    let answer_path = output_dir.join(format!("{sample_bundle}-0.ans"));
    let input_path = output_dir.join(format!("{sample_bundle}-0.in"));
    check_sample_checker_sanity(
        report,
        work_dir,
        problem,
        &input_path,
        &answer_path,
        &output_dir,
    );
    let answer = if answer_path.is_file() {
        match std::fs::read_to_string(&answer_path) {
            Ok(answer) => Some(answer),
            Err(err) => {
                report.warning(
                    codes::SAMPLE_ANSWER_UNREADABLE,
                    format!("generated sample-0.ans could not be read: {err}"),
                    None,
                );
                None
            }
        }
    } else {
        None
    };
    let _ = std::fs::remove_dir_all(&output_dir);
    answer
}

fn check_sample_checker_sanity(
    report: &mut CheckReport,
    work_dir: &Path,
    problem: &Problem,
    input_path: &Path,
    answer_path: &Path,
    output_dir: &Path,
) {
    if problem.checker_name.is_none() || !input_path.is_file() || !answer_path.is_file() {
        return;
    }
    let checker_report_path = output_dir.join("checker-sanity.txt");
    match run_configured_checker_on_files(
        work_dir,
        problem,
        input_path,
        answer_path,
        answer_path,
        &checker_report_path,
        DEFAULT_OUTPUT_LIMIT_BYTES,
    ) {
        Ok(Some(run)) if run.result.ok => {}
        Ok(Some(run)) => {
            let mut message = format!(
                "checker `{}` rejected generated sample answer used as participant output: {}",
                run.checker,
                run.result.status_line()
            );
            if let Some(report_text) = run.report
                && !report_text.trim().is_empty()
            {
                message.push_str("; report: ");
                message.push_str(report_text.trim());
            }
            report.error(
                codes::CHECKER_SANITY_FAILED,
                message,
                Some(work_dir.join("problem.yaml")),
            );
        }
        Ok(None) => {}
        Err(err) => report.error(
            codes::CHECKER_SANITY_FAILED,
            format!("checker sanity failed to run on generated sample: {err:#}"),
            Some(work_dir.join("problem.yaml")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::schema::{
        CppProgram, Program, Test, TestBundle, TestCase, TestTask, TestTaskType,
    };
    use crate::tool::{init_package, temp_test_dir};
    use std::collections::HashMap;

    #[test]
    fn report_render_separates_errors_and_warnings() {
        let mut report = CheckReport::new(PathBuf::from("pkg"));
        report.error("bad", "broken", Some(PathBuf::from("problem.yaml")));
        report.warning("soft", "suspicious", None);

        let rendered = report.render_text();

        assert!(rendered.contains("status: `FAIL`"));
        assert!(rendered.contains("## errors"));
        assert!(rendered.contains("[bad] broken"));
        assert!(rendered.contains("## warnings"));
        assert!(rendered.contains("[soft] suspicious"));
    }

    #[test]
    fn markdown_parser_finds_sample_output_block() {
        let blocks = markdown_sample_output_blocks(
            "# Sample Input\n```text\n1 2\n```\n# Sample Output\n```text\n3\n```\n",
        );

        assert_eq!(blocks, vec!["3\n"]);
    }

    #[test]
    fn sample_output_normalization_ignores_crlf_and_trailing_space() {
        assert_eq!(
            normalize_output_block("3  \r\n4\t\r\n\r\n"),
            normalize_output_block("3\n4\n")
        );
    }

    #[test]
    fn sample_answer_path_uses_samples_bundle_when_sample_is_absent() {
        let root = temp_test_dir("cptool-check-samples-fallback");
        let data_dir = root.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let answer_path = data_dir.join("samples-0.ans");
        std::fs::write(&answer_path, "42\n").unwrap();
        let problem = problem_with_bundles(["samples"]);

        assert_eq!(
            sample_answer_from_data_dir(&root, Some(&problem)),
            Some(answer_path)
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_warns_when_statement_has_multiple_sample_outputs() {
        let root = temp_test_dir("cptool-check-multiple-sample-output");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("statement.md"),
            "# Sample Output\n```text\n1\n```\n# Sample Output\n```text\n2\n```\n",
        )
        .unwrap();
        let mut report = CheckReport::new(root.clone());

        check_statement_sample_output(&mut report, &root, None, Some("1\n"));

        assert_issue(&report, "sample_output_ambiguous", CheckSeverity::Warning);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_reports_missing_required_files() {
        let root = temp_test_dir("cptool-check-missing");
        std::fs::create_dir_all(&root).unwrap();

        let report = check_problem_package(&root);

        assert!(report.has_errors());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "required_file_missing")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn init_package_default_reports_empty_answer_instead_of_generation_failure() {
        let root = temp_test_dir("cptool-check-init");
        let problem_dir = init_package(&root, "Check Me").unwrap();

        let report = check_problem_package(&problem_dir);

        assert!(report.has_errors());
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.code == "sample_generation_failed")
        );
        assert_issue(&report, "empty_answer", CheckSeverity::Error);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_reports_generation_in_progress_and_skips_data_checks() {
        let root = temp_test_dir("cptool-check-in-progress");
        let problem_dir = init_package(&root, "Check In Progress").unwrap();
        let data_dir = problem_dir.join("data");
        let lock_dir = data_dir.join(".cptool-gen.lock");
        std::fs::create_dir_all(&lock_dir).unwrap();

        let report = check_problem_package(&problem_dir);

        assert!(report.has_errors());
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.code == "data_generation_in_progress")
            .expect("expected data generation lock issue");
        assert_eq!(issue.path, Some(lock_dir.clone()));
        assert_eq!(issue.kind.as_deref(), Some("lock"));
        assert_eq!(issue.transient, Some(true));
        assert_eq!(
            issue.retry_after.as_deref(),
            Some("wait_for_generation_then_retry")
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.code == "sample_generation_failed")
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_warns_when_validator_is_missing_without_omitted_reason() {
        let root = temp_test_dir("cptool-check-validator-missing");
        let problem_dir = create_minimal_check_package(&root, None, None);

        let report = check_problem_package(&problem_dir);

        assert_issue(&report, "validator_missing", CheckSeverity::Warning);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_does_not_warn_when_validator_omitted_reason_is_declared() {
        let root = temp_test_dir("cptool-check-validator-reason");
        let problem_dir =
            create_minimal_check_package(&root, None, Some("interactive output is unrestricted"));

        let report = check_problem_package(&problem_dir);

        assert_no_issue(&report, "validator_missing");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_does_not_warn_when_validator_is_declared() {
        let root = temp_test_dir("cptool-check-validator-declared");
        let problem_dir = create_minimal_check_package(&root, Some("val"), None);

        let report = check_problem_package(&problem_dir);

        assert_no_issue(&report, "validator_missing");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_warns_on_unknown_yaml_field() {
        let root = temp_test_dir("cptool-check-unknown-field");
        let problem_dir = create_minimal_check_package(&root, None, None);
        let yaml_path = problem_dir.join("problem.yaml");
        let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
        yaml.push_str("surprise: true\n");
        std::fs::write(&yaml_path, yaml).unwrap();

        let report = check_problem_package(&problem_dir);

        let issue = report
            .issues
            .iter()
            .find(|issue| issue.code == "unknown_field")
            .expect("expected unknown field warning");
        assert_eq!(issue.severity, CheckSeverity::Warning);
        assert_eq!(issue.location.as_deref(), Some("surprise"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_problem_structure_reports_task_and_bundle_issues() {
        let root = temp_test_dir("cptool-check-structure");
        std::fs::create_dir_all(&root).unwrap();
        let mut problem = minimal_problem();
        problem.test.tasks.clear();

        let mut report = CheckReport::new(root.clone());
        check_problem_structure(&mut report, &root, &problem);
        assert_issue(&report, "test_tasks_empty", CheckSeverity::Error);
        assert_issue(&report, "bundle_uncovered_by_task", CheckSeverity::Warning);

        problem = minimal_problem();
        problem.test.bundles.get_mut("main").unwrap().cases.clear();
        problem.test.tasks[0].bundles.clear();
        problem.test.tasks[0].score = 42.0;
        let mut report = CheckReport::new(root.clone());
        check_problem_structure(&mut report, &root, &problem);
        assert_issue(&report, "bundle_empty", CheckSeverity::Error);
        assert_issue(&report, "task_has_no_bundles", CheckSeverity::Error);
        assert_issue(&report, "task_score_total_not_100", CheckSeverity::Warning);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_problem_structure_reports_dependency_cycles() {
        let root = temp_test_dir("cptool-check-dependency-cycle");
        std::fs::create_dir_all(&root).unwrap();
        let mut problem = minimal_problem();
        problem.test.tasks.push(TestTask {
            name: "extra".to_string(),
            score: 0.0,
            task_type: TestTaskType::Min,
            bundles: vec!["main".to_string()],
            dependencies: vec!["main".to_string()],
        });
        problem.test.tasks[0].dependencies = vec!["extra".to_string()];

        let mut report = CheckReport::new(root.clone());
        check_problem_structure(&mut report, &root, &problem);

        assert_issue(&report, "task_dependency_cycle", CheckSeverity::Error);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_stress_plans_reports_quality_and_shape_issues() {
        let root = temp_test_dir("cptool-check-stress");
        std::fs::create_dir_all(&root).unwrap();
        let mut problem = minimal_problem();

        let mut report = CheckReport::new(root.clone());
        check_stress_plans(&mut report, &root, &problem);
        assert_issue(&report, "stress_plans_missing", CheckSeverity::Warning);

        problem.stress.plans.push(crate::tool::schema::StressPlan {
            name: "bad".to_string(),
            generator: "missing_gen.py".to_string(),
            args: Vec::new(),
            against: vec!["std".to_string()],
            cases: 0,
            seed_base: None,
            expect: crate::tool::schema::StressPlanExpectation::Fail,
        });
        let mut report = CheckReport::new(root.clone());
        check_stress_plans(&mut report, &root, &problem);

        assert_issue(
            &report,
            "stress_positive_plan_missing",
            CheckSeverity::Warning,
        );
        assert_issue(&report, "stress_plan_empty", CheckSeverity::Error);
        assert_issue(&report, "stress_plan_against_too_few", CheckSeverity::Error);
        assert_issue(&report, "stress_plan_program_missing", CheckSeverity::Error);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn check_reports_text_audit_issues() {
        let root = temp_test_dir("cptool-check-text-audit");
        let problem_dir = create_minimal_check_package(&root, None, Some("simple input"));
        let yaml_path = problem_dir.join("problem.yaml");
        let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
        yaml.push_str(
            "stress:\n  plans:\n  - name: wrong-proof\n    generator: gen\n    against: [std, gen]\n    cases: 1\n    expect: fail\n",
        );
        std::fs::write(&yaml_path, yaml).unwrap();
        std::fs::create_dir_all(problem_dir.join("pkg")).unwrap();
        std::fs::write(
            problem_dir.join("pkg").join("problem.yaml"),
            "name: nested\n",
        )
        .unwrap();
        std::fs::write(
            problem_dir.join("quality_report.md"),
            "正向覆盖 wrong-proof\nmissing tests/failures/nope.txt\nrate limit\n",
        )
        .unwrap();

        let report = check_problem_package(&problem_dir);

        assert_issue(&report, "placeholder_text", CheckSeverity::Warning);
        assert_issue(&report, "double_nested_problem_dir", CheckSeverity::Warning);
        assert_issue(&report, "service_side_noise", CheckSeverity::Warning);
        assert_issue(&report, "missing_failure_reference", CheckSeverity::Warning);
        assert_issue(
            &report,
            "negative_plan_counted_as_positive",
            CheckSeverity::Warning,
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    fn create_minimal_check_package(
        root: &Path,
        validator: Option<&str>,
        omitted_reason: Option<&str>,
    ) -> PathBuf {
        let problem_dir = root.join("pkg");
        let src_dir = problem_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(problem_dir.join("statement.md"), "# Statement\n").unwrap();
        std::fs::write(problem_dir.join("editorial.md"), "# Editorial\n").unwrap();
        std::fs::write(src_dir.join("std.cpp"), "int main(){}\n").unwrap();
        std::fs::write(src_dir.join("gen.cpp"), "int main(){}\n").unwrap();
        if validator.is_some() {
            std::fs::write(src_dir.join("val.cpp"), "int main(){}\n").unwrap();
        }

        let mut yaml = String::from(
            "name: Validator Check\nprograms:\n  gen:\n    info: !cpp\n      path: ./src/gen.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\n  std:\n    info: !cpp\n      path: ./src/std.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\n",
        );
        if validator.is_some() {
            yaml.push_str(
                "  val:\n    info: !cpp\n      path: ./src/val.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\n",
            );
        }
        yaml.push_str("solution: std\n");
        if let Some(validator) = validator {
            yaml.push_str(&format!("validator: {validator}\n"));
        }
        if let Some(reason) = omitted_reason {
            yaml.push_str(&format!("validator_omitted_reason: {reason:?}\n"));
        }
        yaml.push_str(
            "test:\n  bundles:\n    main:\n      cases:\n      - generator: gen\n        args: []\n  tasks:\n  - name: main\n    score: 100.0\n    type: min\n    bundles: [main]\n",
        );
        std::fs::write(problem_dir.join("problem.yaml"), yaml).unwrap();

        problem_dir
    }

    fn assert_issue(report: &CheckReport, code: &str, severity: CheckSeverity) {
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == code && issue.severity == severity)
        );
    }

    fn assert_no_issue(report: &CheckReport, code: &str) {
        assert!(!report.issues.iter().any(|issue| issue.code == code));
    }

    fn minimal_problem() -> Problem {
        problem_with_bundles(["main"])
    }

    fn problem_with_bundles<const N: usize>(bundle_names: [&str; N]) -> Problem {
        let mut programs = HashMap::new();
        programs.insert(
            "gen".to_string(),
            Program {
                info: ProgramInfo::Cpp(CppProgram {
                    path: PathBuf::from("src/gen.cpp"),
                    compile_args: Vec::new(),
                }),
                time_limit_secs: 1.0,
                memory_limit_mb: 512.0,
            },
        );
        programs.insert(
            "std".to_string(),
            Program {
                info: ProgramInfo::Cpp(CppProgram {
                    path: PathBuf::from("src/std.cpp"),
                    compile_args: Vec::new(),
                }),
                time_limit_secs: 1.0,
                memory_limit_mb: 512.0,
            },
        );

        let bundles = bundle_names
            .into_iter()
            .map(|name| {
                (
                    name.to_string(),
                    TestBundle {
                        cases: vec![TestCase {
                            generator_name: "gen".to_string(),
                            args: Vec::new(),
                        }],
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        Problem {
            name: "samples fallback".to_string(),
            time_limit_secs: 1.0,
            memory_limit_mb: 512.0,
            cpp_compile_args: crate::tool::schema::default_compile_args(),
            output: Default::default(),
            generator_name: Some("gen".to_string()),
            stress: Default::default(),
            programs,
            test: Test {
                bundles,
                tasks: vec![TestTask {
                    name: bundle_names[0].to_string(),
                    score: 100.0,
                    task_type: TestTaskType::Min,
                    bundles: vec![bundle_names[0].to_string()],
                    dependencies: Vec::new(),
                }],
            },
            solution_name: "std".to_string(),
            validator_name: None,
            validator_omitted_reason: None,
            checker_name: None,
        }
    }
}
