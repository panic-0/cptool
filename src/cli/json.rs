use cptool::support::count_lines;
use cptool::tool;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;

#[derive(Serialize)]
pub(super) struct RunJsonSummary<'a> {
    label: &'a str,
    verdict: &'a str,
    phase: &'a str,
    reason_code: &'a str,
    exit_code: Option<i32>,
    diagnostic: Option<&'a str>,
    elapsed_ms: u128,
    stdout_bytes: usize,
    stdout_lines: usize,
    stdout_sha256: String,
    stderr_bytes: usize,
    stderr_nonempty: bool,
    truncated_stdout: bool,
    truncated_stderr: bool,
    compile: &'a tool::CompileReport,
}

impl<'a> From<&'a tool::RunResult> for RunJsonSummary<'a> {
    fn from(result: &'a tool::RunResult) -> Self {
        Self {
            label: &result.label,
            verdict: &result.verdict,
            phase: &result.phase,
            reason_code: &result.reason_code,
            exit_code: result.exit_code,
            diagnostic: result.diagnostic.as_deref(),
            elapsed_ms: result.elapsed_ms,
            stdout_bytes: result.stdout_bytes.len(),
            stdout_lines: count_lines(&result.stdout_bytes),
            stdout_sha256: format!("{:x}", Sha256::digest(&result.stdout_bytes)),
            stderr_bytes: result.stderr_bytes.len(),
            stderr_nonempty: !result.stderr_bytes.is_empty(),
            truncated_stdout: result.truncated_stdout,
            truncated_stderr: result.truncated_stderr,
            compile: &result.compile,
        }
    }
}

#[derive(Serialize)]
pub(super) struct JudgeJsonSummary<'a> {
    role: &'static str,
    program: &'a str,
    expect: &'static str,
    ok: bool,
    observed: &'static str,
    run: RunJsonSummary<'a>,
    report_path: Option<&'a PathBuf>,
    report: Option<&'a str>,
    warnings: &'a [tool::JudgeWarning],
}

#[derive(Serialize)]
pub(super) struct JudgeBatchJsonSummary<'a> {
    role: &'static str,
    ok: bool,
    total: usize,
    passed: usize,
    failed: usize,
    fixtures: Vec<JudgeFixtureJsonSummary<'a>>,
}

impl<'a> JudgeBatchJsonSummary<'a> {
    pub(super) fn new(role: &'static str, fixtures: Vec<JudgeFixtureJsonSummary<'a>>) -> Self {
        let total = fixtures.len();
        let passed = fixtures.iter().filter(|fixture| fixture.report.ok).count();
        Self {
            role,
            ok: passed == total,
            total,
            passed,
            failed: total - passed,
            fixtures,
        }
    }
}

#[derive(Serialize)]
pub(super) struct JudgeFixtureJsonSummary<'a> {
    pub(super) name: &'a str,
    pub(super) path: &'a PathBuf,
    pub(super) report: JudgeJsonSummary<'a>,
}

impl<'a> From<&'a tool::JudgeReport> for JudgeJsonSummary<'a> {
    fn from(report: &'a tool::JudgeReport) -> Self {
        Self {
            role: report.kind.as_str(),
            program: &report.program,
            expect: report.expect.as_str(),
            ok: report.ok,
            observed: report.observed.as_str(),
            run: RunJsonSummary::from(&report.run),
            report_path: report.report_path.as_ref(),
            report: report.report.as_deref(),
            warnings: &report.warnings,
        }
    }
}

#[derive(Serialize)]
pub(super) struct TaskJsonReport<'a> {
    tasks: Vec<TaskJsonSummary<'a>>,
}

impl<'a> TaskJsonReport<'a> {
    pub(super) fn from_summaries(summaries: &'a [tool::StressSummary]) -> Self {
        Self {
            tasks: summaries.iter().map(TaskJsonSummary::from).collect(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct BatchJsonReport<'a> {
    checks: Vec<BatchJsonSummary<'a>>,
}

impl<'a> BatchJsonReport<'a> {
    pub(super) fn from_summaries(summaries: &'a [tool::StressSummary]) -> Self {
        Self {
            checks: summaries.iter().map(BatchJsonSummary::from).collect(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct TaskJsonSummary<'a> {
    task_name: Option<&'a str>,
    #[serde(flatten)]
    summary: StressJsonFields<'a>,
}

impl<'a> From<&'a tool::StressSummary> for TaskJsonSummary<'a> {
    fn from(summary: &'a tool::StressSummary) -> Self {
        Self {
            task_name: summary.plan_name.as_deref(),
            summary: StressJsonFields::from(summary),
        }
    }
}

#[derive(Serialize)]
pub(super) struct BatchJsonSummary<'a> {
    check_name: Option<&'a str>,
    #[serde(flatten)]
    summary: StressJsonFields<'a>,
}

impl<'a> From<&'a tool::StressSummary> for BatchJsonSummary<'a> {
    fn from(summary: &'a tool::StressSummary) -> Self {
        Self {
            check_name: summary.plan_name.as_deref(),
            summary: StressJsonFields::from(summary),
        }
    }
}

#[derive(Serialize)]
struct StressJsonFields<'a> {
    checker: Option<&'a str>,
    answer_program: Option<&'a str>,
    cases: usize,
    elapsed_ms: u128,
    against: &'a [String],
    empty_stdout_cases: usize,
    all_empty_stdout_cases: usize,
    unique_input_hashes: usize,
    expected_failure: Option<&'a tool::ExpectedStressFailure>,
    warnings: Vec<tool::StressWarning>,
}

impl<'a> From<&'a tool::StressSummary> for StressJsonFields<'a> {
    fn from(summary: &'a tool::StressSummary) -> Self {
        Self {
            checker: summary.checker.as_deref(),
            answer_program: summary.answer_program.as_deref(),
            cases: summary.cases,
            elapsed_ms: summary.elapsed_ms,
            against: &summary.against,
            empty_stdout_cases: summary.empty_stdout_cases,
            all_empty_stdout_cases: summary.all_empty_stdout_cases,
            unique_input_hashes: summary.unique_input_hashes,
            expected_failure: summary.expected_failure.as_ref(),
            warnings: summary.warnings(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct CheckJsonReport<'a> {
    work_dir: &'a PathBuf,
    status: &'static str,
    errors: usize,
    warnings: usize,
    issues: &'a [tool::CheckIssue],
}

impl<'a> From<&'a tool::CheckReport> for CheckJsonReport<'a> {
    fn from(report: &'a tool::CheckReport) -> Self {
        Self {
            work_dir: &report.work_dir,
            status: if report.has_errors() { "fail" } else { "pass" },
            errors: report.error_count(),
            warnings: report.warning_count(),
            issues: &report.issues,
        }
    }
}

pub(super) fn print<T: Serialize>(value: &T) -> anyhow::Result<()> {
    std::io::stdout().lock().write_all(&to_bytes(value)?)?;
    Ok(())
}

pub(super) fn to_bytes<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
