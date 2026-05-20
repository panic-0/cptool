use super::problem::{load_problem, normalize_work_dir, resolve_path};
use super::program::{ProgramSpec, resolve_named_or_source, run_spec};
use super::schema::RunResult;
use super::stress_args::direct_stress_args_by_case;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn stress(
    work_dir: &Path,
    generator: &str,
    against: &[String],
    cases: usize,
    args: &[String],
    failure_dir: Option<&Path>,
    output_limit_bytes: usize,
) -> Result<()> {
    stress_with_summary(
        work_dir,
        generator,
        against,
        cases,
        args,
        failure_dir,
        output_limit_bytes,
    )?;
    Ok(())
}

pub fn stress_with_summary(
    work_dir: &Path,
    generator: &str,
    against: &[String],
    cases: usize,
    args: &[String],
    failure_dir: Option<&Path>,
    output_limit_bytes: usize,
) -> Result<StressSummary> {
    stress_with_options(StressOptions {
        work_dir,
        generator,
        against,
        cases,
        args,
        failure_dir,
        output_limit_bytes,
        print_progress: true,
        print_warnings: true,
    })
}

pub struct StressOptions<'a> {
    pub work_dir: &'a Path,
    pub generator: &'a str,
    pub against: &'a [String],
    pub cases: usize,
    pub args: &'a [String],
    pub failure_dir: Option<&'a Path>,
    pub output_limit_bytes: usize,
    pub print_progress: bool,
    pub print_warnings: bool,
}

pub fn stress_with_options(options: StressOptions<'_>) -> Result<StressSummary> {
    let args_by_case = direct_stress_args_by_case(options.args, options.cases);
    run_stress(StressRunOptions {
        work_dir: options.work_dir,
        generator: options.generator,
        against: options.against,
        args_by_case,
        failure_dir: options.failure_dir,
        output_limit_bytes: options.output_limit_bytes,
        plan_name: None,
        print_progress: options.print_progress,
        print_warnings: options.print_warnings,
        expect_failure: false,
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct StressSummary {
    pub plan_name: Option<String>,
    pub cases: usize,
    pub elapsed_ms: u128,
    pub against: Vec<String>,
    pub empty_stdout_cases: usize,
    pub all_empty_stdout_cases: usize,
    pub unique_input_hashes: usize,
    pub expected_failure: Option<ExpectedStressFailure>,
}

impl StressSummary {
    pub fn warnings(&self) -> Vec<StressWarning> {
        let mut warnings = Vec::new();
        if self.all_empty_stdout_cases > 0 {
            warnings.push(StressWarning {
                code: "all_empty_output",
                count: self.all_empty_stdout_cases,
                random_coverage: None,
            });
        }
        if self.has_repeated_input_warning() {
            warnings.push(StressWarning {
                code: "repeated_input",
                count: 1,
                random_coverage: Some(false),
            });
        }
        warnings
    }

    pub fn summary_line(&self) -> String {
        let name = self.plan_name.as_deref().unwrap_or("stress");
        if let Some(failure) = &self.expected_failure {
            return format!(
                "{name}: expected_fail observed=true case={} reason={} failed_cases={} passed_cases={} failure_ratio={:.3} cases_run={} unique_input_hashes={} against={} elapsed={}ms warnings={}",
                failure.case_index,
                failure.reason,
                failure.failed_cases,
                failure.passed_cases,
                failure.failure_ratio,
                self.cases,
                self.unique_input_hashes,
                self.against.join(","),
                self.elapsed_ms,
                self.warning_summary(),
            );
        }
        format!(
            "{name}: ok cases={} unique_input_hashes={} against={} elapsed={}ms empty_stdout_cases={} all_empty_stdout_cases={} warnings={}",
            self.cases,
            self.unique_input_hashes,
            self.against.join(","),
            self.elapsed_ms,
            self.empty_stdout_cases,
            self.all_empty_stdout_cases,
            self.warning_summary()
        )
    }

    fn warning_summary(&self) -> String {
        let warnings = self
            .warnings()
            .into_iter()
            .map(|warning| format!("{}:{}", warning.code, warning.count))
            .collect::<Vec<_>>();
        if warnings.is_empty() {
            "0".to_string()
        } else {
            warnings.join(",")
        }
    }

    fn has_repeated_input_warning(&self) -> bool {
        self.cases > 1 && self.unique_input_hashes == 1
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StressWarning {
    pub code: &'static str,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub random_coverage: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ExpectedStressFailure {
    pub case_index: usize,
    pub reason: String,
    pub failed_cases: usize,
    pub passed_cases: usize,
    pub failure_ratio: f64,
    pub input_sha256: String,
    pub input_path: PathBuf,
    pub report_path: PathBuf,
    pub outputs: Vec<ExpectedStressOutput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ExpectedStressOutput {
    pub label: String,
    pub status_line: String,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
}

pub(crate) struct StressRunOptions<'a> {
    pub(crate) work_dir: &'a Path,
    pub(crate) generator: &'a str,
    pub(crate) against: &'a [String],
    pub(crate) args_by_case: Vec<Vec<String>>,
    pub(crate) failure_dir: Option<&'a Path>,
    pub(crate) output_limit_bytes: usize,
    pub(crate) plan_name: Option<&'a str>,
    pub(crate) print_progress: bool,
    pub(crate) print_warnings: bool,
    pub(crate) expect_failure: bool,
}

pub(crate) fn run_stress(options: StressRunOptions<'_>) -> Result<StressSummary> {
    let StressRunOptions {
        work_dir,
        generator,
        against,
        args_by_case,
        failure_dir,
        output_limit_bytes,
        plan_name,
        print_progress,
        print_warnings,
        expect_failure,
    } = options;
    let cases = args_by_case.len();
    if against.len() < 2 {
        anyhow::bail!("stress requires at least two --against programs or sources");
    }
    let work_dir = normalize_work_dir(work_dir)?;
    let problem = load_problem(&work_dir)?;
    let generator = resolve_named_or_source(&work_dir, &problem, generator)?;
    let targets = against
        .iter()
        .map(|item| resolve_named_or_source(&work_dir, &problem, item))
        .collect::<Result<Vec<_>>>()?;
    let failure_dir = failure_dir
        .map(|path| resolve_path(&work_dir, path))
        .unwrap_or_else(|| work_dir.join("tests").join("failures"));

    let start = Instant::now();
    let mut input_hashes = HashSet::new();
    let mut empty_stdout_cases = 0;
    let mut all_empty_stdout_cases = 0;
    let mut failed_cases = 0usize;
    let mut expected_failure = None;
    for (case0, args) in args_by_case.iter().enumerate() {
        let index = case0 + 1;
        let outcome = run_stress_case(
            &work_dir,
            &generator,
            &targets,
            args,
            index,
            output_limit_bytes,
        )?;
        input_hashes.insert(outcome.input_hash.clone());
        if let Some(failure) = outcome.failure {
            if expect_failure {
                let case_index = failure.case_index;
                let reason = failure.reason.clone();
                failed_cases += 1;
                if expected_failure.is_none() {
                    let artifacts =
                        save_stress_failure_artifacts(&failure_dir, plan_name, &failure)?;
                    expected_failure = Some(ExpectedStressFailure {
                        case_index,
                        reason,
                        failed_cases: 0,
                        passed_cases: 0,
                        failure_ratio: 0.0,
                        input_sha256: hex_bytes(&outcome.input_hash),
                        input_path: artifacts.input_path,
                        report_path: artifacts.report_path,
                        outputs: artifacts
                            .outputs
                            .into_iter()
                            .map(ExpectedStressOutput::from)
                            .collect(),
                    });
                }
                if print_progress {
                    if let Some(plan_name) = plan_name {
                        println!("plan `{plan_name}` case {case_index} expected failure observed");
                    } else {
                        println!("case {case_index} expected failure observed");
                    }
                }
                continue;
            }
            save_stress_failure(&failure_dir, plan_name, failure)?;
            unreachable!("save_stress_failure always returns an error");
        }
        if outcome.empty_stdout {
            empty_stdout_cases += 1;
        }
        if outcome.all_empty_stdout {
            all_empty_stdout_cases += 1;
            if print_warnings {
                eprintln!(
                    "warning: all_empty_output case={} against={} input_bytes={}",
                    index,
                    against.join(","),
                    outcome.input_bytes
                );
            }
        }
        if print_progress {
            if let Some(plan_name) = plan_name {
                println!("plan `{plan_name}` case {index} ok");
            } else {
                println!("case {index} ok");
            }
        }
    }
    let mut summary = StressSummary {
        plan_name: plan_name.map(str::to_string),
        cases,
        elapsed_ms: start.elapsed().as_millis(),
        against: against.to_vec(),
        empty_stdout_cases,
        all_empty_stdout_cases,
        unique_input_hashes: input_hashes.len(),
        expected_failure: None,
    };
    if expect_failure {
        let Some(mut failure) = expected_failure else {
            let plan = plan_name
                .map(|name| format!(" plan `{name}`"))
                .unwrap_or_default();
            anyhow::bail!(
                "stress{plan} expected failure but all {} cases passed",
                summary.cases
            );
        };
        failure.failed_cases = failed_cases;
        failure.passed_cases = cases.saturating_sub(failed_cases);
        failure.failure_ratio = if cases == 0 {
            0.0
        } else {
            failed_cases as f64 / cases as f64
        };
        summary.expected_failure = Some(failure);
        return Ok(summary);
    }
    if print_warnings && summary.has_repeated_input_warning() {
        eprintln!(
            "warning: repeated_input cases={} unique_input_hashes=1 random_coverage=false hint=generator_args_produced_identical_inputs",
            summary.cases
        );
    }
    Ok(summary)
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct StressCaseOutcome {
    failure: Option<StressFailure>,
    input_hash: Vec<u8>,
    input_bytes: usize,
    empty_stdout: bool,
    all_empty_stdout: bool,
}

struct StressFailure {
    case_index: usize,
    reason: String,
    input: Vec<u8>,
    results: Vec<RunResult>,
}

struct StressOutputArtifact {
    label: String,
    status_line: String,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

struct SavedStressFailureArtifacts {
    stem: PathBuf,
    input_path: PathBuf,
    report_path: PathBuf,
    outputs: Vec<StressOutputArtifact>,
}

impl From<StressOutputArtifact> for ExpectedStressOutput {
    fn from(artifact: StressOutputArtifact) -> Self {
        Self {
            label: artifact.label,
            status_line: artifact.status_line,
            stdout_path: artifact.stdout_path,
            stderr_path: artifact.stderr_path,
        }
    }
}

fn run_stress_case(
    work_dir: &Path,
    generator: &ProgramSpec,
    targets: &[ProgramSpec],
    args: &[String],
    index: usize,
    output_limit_bytes: usize,
) -> Result<StressCaseOutcome> {
    let gen_result = run_spec(work_dir, generator, args, None, output_limit_bytes)?;
    if !gen_result.ok {
        anyhow::bail!(
            "{}",
            gen_result.failure_report(&format!("generator failed on stress case {index}"))
        );
    }
    if gen_result.truncated_stdout {
        anyhow::bail!(
            "generator output on stress case {index} exceeded --output-limit-bytes ({output_limit_bytes})"
        );
    }
    let input = gen_result.stdout_bytes;
    let input_hash = Sha256::digest(&input).to_vec();
    let mut results = Vec::new();
    for target in targets {
        let result = run_spec(work_dir, target, &[], Some(&input), output_limit_bytes)?;
        if result.truncated_stdout {
            anyhow::bail!(
                "program `{}` output on stress case {index} exceeded --output-limit-bytes ({output_limit_bytes})",
                result.label
            );
        }
        results.push(result);
    }
    let input_bytes = input.len();
    if let Some(reason) = classify_stress_failure(&results) {
        return Ok(StressCaseOutcome {
            failure: Some(StressFailure {
                case_index: index,
                reason,
                input,
                results,
            }),
            input_hash,
            input_bytes,
            empty_stdout: false,
            all_empty_stdout: false,
        });
    }
    let successful_non_empty_input = input_bytes > 0;
    let empty_stdout = successful_non_empty_input
        && results
            .iter()
            .any(|result| result.ok && result.stdout_bytes.is_empty());
    let all_empty_stdout = successful_non_empty_input
        && results
            .iter()
            .all(|result| result.ok && result.stdout_bytes.is_empty());
    Ok(StressCaseOutcome {
        failure: None,
        input_hash,
        input_bytes,
        empty_stdout,
        all_empty_stdout,
    })
}

fn save_stress_failure(
    failure_dir: &Path,
    plan_name: Option<&str>,
    failure: StressFailure,
) -> Result<()> {
    let artifacts = save_stress_failure_artifacts(failure_dir, plan_name, &failure)?;
    let plan = plan_name
        .map(|name| format!(" plan `{name}`"))
        .unwrap_or_default();
    anyhow::bail!(
        "stress{plan} failed on case {}; {}; saved {}.in, {}.txt, and per-program .out/.err files",
        failure.case_index,
        failure.reason,
        artifacts.stem.display(),
        artifacts.stem.display()
    );
}

fn save_stress_failure_artifacts(
    failure_dir: &Path,
    plan_name: Option<&str>,
    failure: &StressFailure,
) -> Result<SavedStressFailureArtifacts> {
    std::fs::create_dir_all(failure_dir)?;
    let (stem, mut input_file) = create_failure_input(failure_dir, plan_name)?;
    let input_path = stem.with_extension("in");
    let report_path = stem.with_extension("txt");
    input_file.write_all(&failure.input)?;
    let artifacts = write_stress_outputs(&stem, &failure.results)?;
    let report = render_stress_failure(plan_name, failure.case_index, &failure.results, &artifacts);
    std::fs::write(&report_path, report.as_bytes())?;
    Ok(SavedStressFailureArtifacts {
        stem,
        input_path,
        report_path,
        outputs: artifacts,
    })
}

fn create_failure_input(
    failure_dir: &Path,
    plan_name: Option<&str>,
) -> Result<(PathBuf, std::fs::File)> {
    let prefix = plan_name
        .map(|name| format!("stress-{}", sanitize_artifact_label(name)))
        .unwrap_or_else(|| "stress".to_string());
    for id in 1.. {
        let stem = failure_dir.join(format!("{prefix}-{id:03}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stem.with_extension("in"))
        {
            Ok(file) => return Ok((stem, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!()
}

fn write_stress_outputs(stem: &Path, results: &[RunResult]) -> Result<Vec<StressOutputArtifact>> {
    results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            let artifact_stem = result_artifact_stem(stem, index, &result.label);
            let stdout_path = artifact_stem.with_extension("out");
            let stderr_path = artifact_stem.with_extension("err");
            std::fs::write(&stdout_path, &result.stdout_bytes)?;
            std::fs::write(&stderr_path, &result.stderr_bytes)?;
            Ok(StressOutputArtifact {
                label: result.label.clone(),
                status_line: result.status_line(),
                stdout_path,
                stderr_path,
            })
        })
        .collect()
}

fn result_artifact_stem(stem: &Path, index: usize, label: &str) -> PathBuf {
    let base = stem
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("stress");
    stem.with_file_name(format!(
        "{base}-{}-{}",
        index + 1,
        sanitize_artifact_label(label)
    ))
}

fn sanitize_artifact_label(label: &str) -> String {
    let sanitized = label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "program".to_string()
    } else {
        sanitized
    }
}

pub(crate) fn normalize_output(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!(
            "{}\n",
            trimmed
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

fn render_stress_failure(
    plan_name: Option<&str>,
    case_index: usize,
    results: &[RunResult],
    artifacts: &[StressOutputArtifact],
) -> String {
    let mut report = match plan_name {
        Some(plan_name) => format!("stress plan `{plan_name}` failed on case {case_index}\n\n"),
        None => format!("stress failed on case {case_index}\n\n"),
    };
    if let Some(reason) = classify_stress_failure(results) {
        report.push_str(&format!("reason: {reason}\n\n"));
    }
    for artifact in artifacts {
        report.push_str(&format!("[{}] {}\n", artifact.label, artifact.status_line));
        report.push_str(&format!("stdout: {}\n", artifact.stdout_path.display()));
        report.push_str(&format!("stderr: {}\n\n", artifact.stderr_path.display()));
    }
    report
}

pub(crate) fn classify_stress_failure(results: &[RunResult]) -> Option<String> {
    if let Some(result) = results.iter().find(|result| !result.ok) {
        return Some(format!("program_failed: {}", result.status_line()));
    }

    let baseline = results.first()?;
    let baseline_output = normalize_output(&baseline.stdout);
    results.iter().skip(1).find_map(|result| {
        (normalize_output(&result.stdout) != baseline_output).then(|| {
            format!(
                "wrong_answer: output mismatch between `{}` and `{}`",
                baseline.label, result.label
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::temp_test_dir;

    #[test]
    fn failure_input_stem_includes_sanitized_plan_name() {
        let root = temp_test_dir("cptool-stress-plan-failure");
        std::fs::create_dir_all(&root).unwrap();

        let (stem, _file) = create_failure_input(&root, Some("small cases")).unwrap();

        assert_eq!(stem.file_name().unwrap(), "stress-small-cases-001");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failure_report_mentions_plan_name() {
        let report = render_stress_failure(Some("random"), 2, &[], &[]);

        assert!(report.starts_with("stress plan `random` failed on case 2"));
    }

    #[test]
    fn stress_summary_line_includes_plan_cases_against_and_elapsed() {
        let summary = StressSummary {
            plan_name: Some("small".to_string()),
            cases: 300,
            elapsed_ms: 1240,
            against: vec!["std".to_string(), "brute".to_string()],
            empty_stdout_cases: 0,
            all_empty_stdout_cases: 0,
            unique_input_hashes: 300,
            expected_failure: None,
        };

        assert_eq!(
            summary.summary_line(),
            "small: ok cases=300 unique_input_hashes=300 against=std,brute elapsed=1240ms empty_stdout_cases=0 all_empty_stdout_cases=0 warnings=0"
        );
    }

    #[test]
    fn stress_summary_line_reports_all_empty_output_warning_count() {
        let summary = StressSummary {
            plan_name: Some("small".to_string()),
            cases: 3,
            elapsed_ms: 7,
            against: vec!["std".to_string(), "brute".to_string()],
            empty_stdout_cases: 3,
            all_empty_stdout_cases: 3,
            unique_input_hashes: 1,
            expected_failure: None,
        };

        assert_eq!(
            summary.summary_line(),
            "small: ok cases=3 unique_input_hashes=1 against=std,brute elapsed=7ms empty_stdout_cases=3 all_empty_stdout_cases=3 warnings=all_empty_output:3,repeated_input:1"
        );
    }

    #[test]
    fn stress_summary_line_reports_repeated_input_warning() {
        let summary = StressSummary {
            plan_name: Some("small".to_string()),
            cases: 2,
            elapsed_ms: 7,
            against: vec!["std".to_string(), "brute".to_string()],
            empty_stdout_cases: 0,
            all_empty_stdout_cases: 0,
            unique_input_hashes: 1,
            expected_failure: None,
        };

        assert_eq!(
            summary.summary_line(),
            "small: ok cases=2 unique_input_hashes=1 against=std,brute elapsed=7ms empty_stdout_cases=0 all_empty_stdout_cases=0 warnings=repeated_input:1"
        );
    }

    #[test]
    fn stress_summary_line_does_not_report_repeated_input_for_single_case() {
        let summary = StressSummary {
            plan_name: Some("small".to_string()),
            cases: 1,
            elapsed_ms: 7,
            against: vec!["std".to_string(), "brute".to_string()],
            empty_stdout_cases: 0,
            all_empty_stdout_cases: 0,
            unique_input_hashes: 1,
            expected_failure: None,
        };

        assert_eq!(
            summary.summary_line(),
            "small: ok cases=1 unique_input_hashes=1 against=std,brute elapsed=7ms empty_stdout_cases=0 all_empty_stdout_cases=0 warnings=0"
        );
    }
}
