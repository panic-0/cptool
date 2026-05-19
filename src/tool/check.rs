use super::data::{data_generation_status, generate_data};
use super::problem::{load_problem, normalize_work_dir, resolve_path};
use super::schema::{DEFAULT_OUTPUT_LIMIT_BYTES, Problem, ProgramInfo};
use super::temp_suffix;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckSeverity {
    Warning,
    Error,
}

impl CheckSeverity {
    fn label(self) -> &'static str {
        match self {
            CheckSeverity::Warning => "warning",
            CheckSeverity::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckIssue {
    pub severity: CheckSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct CheckReport {
    pub work_dir: PathBuf,
    pub issues: Vec<CheckIssue>,
}

impl CheckReport {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            work_dir,
            issues: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == CheckSeverity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == CheckSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == CheckSeverity::Warning)
            .count()
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        let status = if self.has_errors() { "FAIL" } else { "PASS" };
        let _ = writeln!(out, "# cptool check report");
        let _ = writeln!(out);
        let _ = writeln!(out, "- work_dir: `{}`", self.work_dir.display());
        let _ = writeln!(out, "- status: `{status}`");
        let _ = writeln!(out, "- errors: {}", self.error_count());
        let _ = writeln!(out, "- warnings: {}", self.warning_count());
        self.render_group(&mut out, CheckSeverity::Error);
        self.render_group(&mut out, CheckSeverity::Warning);
        out
    }

    fn render_group(&self, out: &mut String, severity: CheckSeverity) {
        let issues = self
            .issues
            .iter()
            .filter(|issue| issue.severity == severity)
            .collect::<Vec<_>>();
        let _ = writeln!(out);
        let _ = writeln!(out, "## {}s", severity.label());
        if issues.is_empty() {
            let _ = writeln!(out, "- none");
            return;
        }
        for issue in issues {
            let _ = write!(out, "- [{}] {}", issue.code, issue.message);
            if let Some(path) = &issue.path {
                let _ = write!(out, " (`{}`)", path.display());
            }
            let _ = writeln!(out);
        }
    }

    fn push(
        &mut self,
        severity: CheckSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) {
        self.issues.push(CheckIssue {
            severity,
            code: code.into(),
            message: message.into(),
            path,
        });
    }

    fn error(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) {
        self.push(CheckSeverity::Error, code, message, path);
    }

    fn warning(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) {
        self.push(CheckSeverity::Warning, code, message, path);
    }
}

pub fn check_problem_package(work_dir: &Path) -> CheckReport {
    let work_dir = normalize_work_dir(work_dir).unwrap_or_else(|_| work_dir.to_path_buf());
    let mut report = CheckReport::new(work_dir.clone());

    check_required_files(&mut report, &work_dir);

    let problem = match load_problem(&work_dir) {
        Ok(problem) => problem,
        Err(err) => {
            report.error(
                "problem_yaml_invalid",
                format!("problem.yaml could not be loaded or validated: {err:#}"),
                Some(work_dir.join("problem.yaml")),
            );
            check_statement_sample_output(&mut report, &work_dir, None, None);
            return report;
        }
    };

    check_program_paths(&mut report, &work_dir, &problem);
    if let Some(status) = data_generation_status(&work_dir.join("data")) {
        report.error(
            "data_generation_in_progress",
            "data generation is in progress; skipped data consistency checks to avoid reading partial output",
            Some(status.marker_path),
        );
        return report;
    }

    check_empty_answers(&mut report, &work_dir, &problem);

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
                "required_file_missing",
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
                "program_path_missing",
                format!("program `{name}` path does not exist"),
                Some(path),
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
            "data_dir_unreadable",
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
                "empty_answer",
                ".ans file is empty but output.allow_empty is not declared",
                Some(path),
            ),
            Ok(_) => {}
            Err(err) => report.warning(
                "answer_unreadable",
                format!("could not inspect .ans file: {err}"),
                Some(path),
            ),
        }
    }
}

fn check_sample_generation(
    report: &mut CheckReport,
    work_dir: &Path,
    problem: &Problem,
) -> Option<String> {
    let Some(sample_bundle) = find_sample_bundle(problem) else {
        report.warning(
            "sample_bundle_missing",
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
            "sample_bundle_empty",
            format!("sample bundle `{sample_bundle}` has no cases"),
            Some(work_dir.join("problem.yaml")),
        );
        return None;
    }

    let output_dir = std::env::temp_dir().join(format!("cptool-check-{}", temp_suffix()));
    let result = generate_data(
        work_dir,
        Some(sample_bundle),
        None,
        Some(&output_dir),
        DEFAULT_OUTPUT_LIMIT_BYTES,
    );
    let generated = match result {
        Ok(generated) => generated,
        Err(err) => {
            report.error(
                "sample_generation_failed",
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
                "empty_answer",
                "generated sample .ans is empty but output.allow_empty is not declared",
                None,
            );
        }
    }

    let answer_path = output_dir.join(format!("{sample_bundle}-0.ans"));
    let answer = if answer_path.is_file() {
        match std::fs::read_to_string(&answer_path) {
            Ok(answer) => Some(answer),
            Err(err) => {
                report.warning(
                    "sample_answer_unreadable",
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

fn find_sample_bundle(problem: &Problem) -> Option<&str> {
    if problem.test.bundles.contains_key("sample") {
        return Some("sample");
    }
    if problem.test.bundles.contains_key("samples") {
        return Some("samples");
    }
    None
}

fn check_statement_sample_output(
    report: &mut CheckReport,
    work_dir: &Path,
    problem: Option<&Problem>,
    generated_sample_answer: Option<&str>,
) {
    let statement_path = work_dir.join("statement.md");
    let Ok(statement) = std::fs::read_to_string(&statement_path) else {
        return;
    };
    let blocks = markdown_sample_output_blocks(&statement);
    if blocks.is_empty() {
        return;
    }
    if blocks.len() > 1 {
        report.warning(
            "sample_output_ambiguous",
            "multiple sample output code blocks were found in statement.md; skipped comparison",
            Some(statement_path),
        );
        return;
    }

    let answer = match generated_sample_answer {
        Some(answer) => answer.to_string(),
        None => {
            let Some(answer_path) = sample_answer_from_data_dir(work_dir, problem) else {
                report.warning(
                    "sample_answer_missing",
                    "sample output was found in statement.md, but sample-0.ans is unavailable",
                    Some(statement_path),
                );
                return;
            };
            let Ok(answer) = std::fs::read_to_string(&answer_path) else {
                report.warning(
                    "sample_answer_unreadable",
                    "sample-0.ans exists but could not be read",
                    Some(answer_path),
                );
                return;
            };
            answer
        }
    };

    if normalize_output_block(&blocks[0]) != normalize_output_block(&answer) {
        report.error(
            "statement_sample_output_mismatch",
            "statement.md sample output does not match sample-0.ans",
            Some(statement_path),
        );
    }
}

fn sample_answer_from_data_dir(work_dir: &Path, problem: Option<&Problem>) -> Option<PathBuf> {
    let bundle = problem.and_then(find_sample_bundle).unwrap_or("sample");
    let path = work_dir.join("data").join(format!("{bundle}-0.ans"));
    path.is_file().then_some(path)
}

fn markdown_sample_output_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = "";
    let mut capture = false;
    let mut current = String::new();
    let mut pending_output = false;

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let is_fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if is_fence {
            let marker = &trimmed[..3];
            if !in_fence {
                in_fence = true;
                fence_marker = marker;
                capture = pending_output;
                pending_output = false;
                current.clear();
            } else if marker == fence_marker {
                in_fence = false;
                if capture {
                    blocks.push(current.clone());
                }
                capture = false;
            } else if capture {
                current.push_str(line);
                current.push('\n');
            }
            continue;
        }

        if in_fence {
            if capture {
                current.push_str(line);
                current.push('\n');
            }
            continue;
        }

        if line.trim().is_empty() {
            continue;
        }
        pending_output = is_sample_output_context(line);
    }

    blocks
}

fn is_sample_output_context(line: &str) -> bool {
    let line = line
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_ascii_lowercase();
    line.contains("sample output")
        || line.contains("output sample")
        || line.contains("样例输出")
        || line.contains("输出样例")
}

fn normalize_output_block(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    lines.trim_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::{init_package, temp_test_dir};

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
    fn init_package_default_reports_empty_std_and_gen_as_generation_error() {
        let root = temp_test_dir("cptool-check-init");
        let problem_dir = init_package(&root, "Check Me").unwrap();

        let report = check_problem_package(&problem_dir);

        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "sample_generation_failed")
        );

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
        assert!(report.issues.iter().any(|issue| {
            issue.code == "data_generation_in_progress" && issue.path == Some(lock_dir.clone())
        }));
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.code == "sample_generation_failed")
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
