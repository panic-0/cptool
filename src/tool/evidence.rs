use super::check::{CheckIssue, CheckOptions, CheckReport, check_problem_package_with_options};
use super::data::{
    GenerateOptions, GenerateReport, GenerateWarning, GenerateWarningKind,
    generate_data_report_with_options,
};
use super::schema::DEFAULT_OUTPUT_LIMIT_BYTES;
use super::stress::StressSummary;
use super::task_expect::{TaskExpectOptions, task_expect_collect_with_options};
use anyhow::Context;
use serde::Deserialize;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct EvidenceOptions {
    pub work_dir: PathBuf,
    pub output_limit_bytes: usize,
    pub skip_gen: bool,
    pub skip_task: bool,
    pub reuse_existing_task: Option<PathBuf>,
    pub generation_lock_timeout: Option<Duration>,
}

impl EvidenceOptions {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            work_dir,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            skip_gen: false,
            skip_task: false,
            reuse_existing_task: None,
            generation_lock_timeout: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EvidenceReport {
    pub cptool_version: String,
    pub work_dir: PathBuf,
    pub check: EvidenceSection<EvidenceCheckReport>,
    #[serde(rename = "gen")]
    pub r#gen: EvidenceSection<GenerateReport>,
    pub task: EvidenceSection<Vec<StressSummary>>,
}

impl EvidenceReport {
    pub fn has_errors(&self) -> bool {
        self.check.is_error()
            || self
                .check
                .report
                .as_ref()
                .is_some_and(EvidenceCheckReport::has_errors)
            || self.r#gen.is_error()
            || self.task.is_error()
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# cptool report evidence\n\n");
        out.push_str(&format!("- cptool_version: `{}`\n", self.cptool_version));
        out.push_str(&format!("- work_dir: `{}`\n", self.work_dir.display()));
        out.push_str(&format!("- check: {}\n", self.check.summary()));
        out.push_str(&format!("- gen: {}\n", self.r#gen.summary()));
        out.push_str(&format!("- task: {}\n", self.task.summary()));
        out
    }

    pub fn render_quality_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("## Tool Evidence\n\n");
        out.push_str(&format!("- cptool_version: `{}`\n", self.cptool_version));
        out.push_str(&format!("- work_dir: `{}`\n\n", self.work_dir.display()));

        out.push_str("### Check\n");
        match (&self.check.status, &self.check.report, &self.check.error) {
            (EvidenceStatus::Ok, Some(report), _) => {
                out.push_str(&format!("- status: `{}`\n", report.status));
                out.push_str(&format!("- errors: {}\n", report.errors));
                out.push_str(&format!("- warnings: {}\n", report.warnings));
            }
            (EvidenceStatus::Skipped, _, Some(reason)) => {
                out.push_str(&format!("- not recorded: {reason}\n"));
            }
            (EvidenceStatus::Error, _, Some(error)) => {
                out.push_str(&format!("- error: {error}\n"));
            }
            _ => out.push_str("- not recorded\n"),
        }

        out.push_str("\n### Generation\n");
        match (&self.r#gen.status, &self.r#gen.report, &self.r#gen.error) {
            (EvidenceStatus::Ok, Some(report), _) => {
                out.push_str(&format!("- cases: {}\n", report.cases));
                out.push_str(&format!("- bundles: {}\n", report.bundles.join(", ")));
                out.push_str(&format!(
                    "- validator_configured: {}\n",
                    report.validator_configured
                ));
                out.push_str(&format!("- validator_calls: {}\n", report.validator_calls));
                out.push_str(&format!(
                    "- warnings: {}\n",
                    generate_warning_summary(&report.warnings)
                ));
            }
            (EvidenceStatus::Skipped, _, Some(reason)) => {
                out.push_str(&format!("- not recorded: {reason}\n"));
            }
            (EvidenceStatus::Error, _, Some(error)) => {
                out.push_str(&format!("- error: {error}\n"));
            }
            _ => out.push_str("- not recorded\n"),
        }

        let task_checks = self
            .task
            .report
            .as_ref()
            .filter(|_| self.task.status == EvidenceStatus::Ok)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let positive = task_checks
            .iter()
            .filter(|check| check.expected_failure.is_none())
            .collect::<Vec<_>>();
        let negative = task_checks
            .iter()
            .filter(|check| check.expected_failure.is_some())
            .collect::<Vec<_>>();

        out.push_str("\n### Positive Task Checks\n");
        render_task_checks(&mut out, &positive, false);
        out.push_str("\n### Negative Task Checks\n");
        render_task_checks(&mut out, &negative, true);
        out
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceStatus {
    Ok,
    Error,
    Skipped,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvidenceCheckReport {
    pub work_dir: PathBuf,
    pub status: &'static str,
    pub errors: usize,
    pub warnings: usize,
    pub issues: Vec<CheckIssue>,
}

impl EvidenceCheckReport {
    fn from_check_report(report: CheckReport) -> Self {
        let errors = report.error_count();
        let warnings = report.warning_count();
        let status = if report.has_errors() { "fail" } else { "pass" };
        Self {
            work_dir: report.work_dir,
            status,
            errors,
            warnings,
            issues: report.issues,
        }
    }

    fn has_errors(&self) -> bool {
        self.errors > 0
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EvidenceSection<T> {
    pub(crate) status: EvidenceStatus,
    pub report: Option<T>,
    pub error: Option<String>,
}

impl<T> EvidenceSection<T> {
    fn ok(report: T) -> Self {
        Self {
            status: EvidenceStatus::Ok,
            report: Some(report),
            error: None,
        }
    }

    fn error(error: impl Into<String>) -> Self {
        Self {
            status: EvidenceStatus::Error,
            report: None,
            error: Some(error.into()),
        }
    }

    fn skipped(reason: impl Into<String>) -> Self {
        Self {
            status: EvidenceStatus::Skipped,
            report: None,
            error: Some(reason.into()),
        }
    }

    fn is_error(&self) -> bool {
        self.status == EvidenceStatus::Error
    }
}

impl EvidenceSection<EvidenceCheckReport> {
    fn summary(&self) -> String {
        match (&self.status, &self.report, &self.error) {
            (EvidenceStatus::Ok, Some(report), _) => format!(
                "ok status={} errors={} warnings={}",
                report.status, report.errors, report.warnings
            ),
            (EvidenceStatus::Error, _, Some(error)) => format!("error {error}"),
            (EvidenceStatus::Skipped, _, Some(reason)) => format!("skipped {reason}"),
            _ => format!("{:?}", self.status),
        }
    }
}

impl EvidenceSection<GenerateReport> {
    fn summary(&self) -> String {
        match (&self.status, &self.report, &self.error) {
            (EvidenceStatus::Ok, Some(report), _) => report.summary_line(),
            (EvidenceStatus::Error, _, Some(error)) => format!("error {error}"),
            (EvidenceStatus::Skipped, _, Some(reason)) => format!("skipped {reason}"),
            _ => format!("{:?}", self.status),
        }
    }
}

impl EvidenceSection<Vec<StressSummary>> {
    fn summary(&self) -> String {
        match (&self.status, &self.report, &self.error) {
            (EvidenceStatus::Ok, Some(report), _) => {
                let cases = report.iter().map(|summary| summary.cases).sum::<usize>();
                format!("ok checks={} cases={}", report.len(), cases)
            }
            (EvidenceStatus::Error, _, Some(error)) => format!("error {error}"),
            (EvidenceStatus::Skipped, _, Some(reason)) => format!("skipped {reason}"),
            _ => format!("{:?}", self.status),
        }
    }
}

pub fn collect_evidence(options: EvidenceOptions) -> EvidenceReport {
    let EvidenceOptions {
        work_dir,
        output_limit_bytes,
        skip_gen,
        skip_task,
        reuse_existing_task,
        generation_lock_timeout,
    } = options;
    let r#gen = if skip_gen {
        EvidenceSection::skipped("requested_by_user")
    } else {
        collect_gen(&work_dir, output_limit_bytes, generation_lock_timeout)
    };
    let check = EvidenceSection::ok(EvidenceCheckReport::from_check_report(
        check_problem_package_with_options(
            &work_dir,
            CheckOptions {
                generation_lock_timeout,
            },
        ),
    ));
    let task = if skip_task {
        EvidenceSection::skipped("requested_by_user")
    } else if let Some(path) = reuse_existing_task {
        collect_reused_task(&path)
    } else {
        collect_task(&work_dir, output_limit_bytes, generation_lock_timeout)
    };

    EvidenceReport {
        cptool_version: env!("CPTOOL_VERSION").to_string(),
        work_dir,
        check,
        r#gen,
        task,
    }
}

#[derive(Deserialize)]
struct TaskJsonReport {
    tasks: Vec<StressSummary>,
}

fn collect_reused_task(path: &Path) -> EvidenceSection<Vec<StressSummary>> {
    match read_reused_task(path) {
        Ok(report) => EvidenceSection::ok(report),
        Err(err) => EvidenceSection::error(err.to_string()),
    }
}

fn read_reused_task(path: &Path) -> anyhow::Result<Vec<StressSummary>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read task JSON `{}`", path.display()))?;
    let report: TaskJsonReport = serde_json::from_str(&text).with_context(|| {
        format!(
            "failed to parse task JSON `{}`; expected output from `cptool test task --summary-only --json`",
            path.display()
        )
    })?;
    Ok(report.tasks)
}

fn collect_gen(
    work_dir: &Path,
    output_limit_bytes: usize,
    generation_lock_timeout: Option<Duration>,
) -> EvidenceSection<GenerateReport> {
    match generate_data_report_with_options(GenerateOptions {
        work_dir: work_dir.to_path_buf(),
        bundle: None,
        selector: None,
        output_dir: None,
        output_limit_bytes,
        generation_lock_timeout,
    }) {
        Ok(report) => EvidenceSection::ok(report),
        Err(err) => EvidenceSection::error(err.to_string()),
    }
}

fn collect_task(
    work_dir: &Path,
    output_limit_bytes: usize,
    generation_lock_timeout: Option<Duration>,
) -> EvidenceSection<Vec<StressSummary>> {
    match task_expect_collect_with_options(TaskExpectOptions {
        work_dir,
        name: None,
        failure_dir: None,
        output_limit_bytes,
        summary_only: true,
        generation_lock_timeout,
    }) {
        Ok(report) => EvidenceSection::ok(report),
        Err(err) => EvidenceSection::error(err.to_string()),
    }
}

fn generate_warning_summary(warnings: &[GenerateWarning]) -> String {
    if warnings.is_empty() {
        return "0".to_string();
    }
    warnings
        .iter()
        .map(|warning| match warning.kind {
            GenerateWarningKind::GeneratorOutputSuspicious => "generator_output_suspicious",
            GenerateWarningKind::EmptyAnswer => "empty_answer",
            GenerateWarningKind::RepeatedInput => "repeated_input",
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn render_task_checks(out: &mut String, checks: &[&StressSummary], negative: bool) {
    if checks.is_empty() {
        out.push_str("- not recorded\n");
        return;
    }
    for check in checks {
        let name = check.plan_name.as_deref().unwrap_or("<unnamed>");
        out.push_str(&format!(
            "- `{name}`: cases={} unique_input_hashes={} warnings={}",
            check.cases,
            check.unique_input_hashes,
            stress_warning_summary(check)
        ));
        if let (Some(checker), Some(answer_program)) = (&check.checker, &check.answer_program) {
            out.push_str(&format!(
                " checker={checker} answer_program={answer_program}"
            ));
        }
        if negative && let Some(failure) = &check.expected_failure {
            out.push_str(&format!(
                " failed_cases={} passed_cases={} failure_ratio={:.3} report={}",
                failure.failed_cases,
                failure.passed_cases,
                failure.failure_ratio,
                failure.report_path.display()
            ));
        }
        out.push('\n');
    }
}

fn stress_warning_summary(summary: &StressSummary) -> String {
    let warnings = summary
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
