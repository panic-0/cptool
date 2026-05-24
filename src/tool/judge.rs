use super::problem::{load_problem, normalize_work_dir, resolve_path};
use super::program::{ProgramSpec, resolve_named_or_source, run_spec};
use super::schema::{DEFAULT_OUTPUT_LIMIT_BYTES, RunResult};
use super::temp_suffix;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeExpectation {
    Pass,
    Fail,
}

impl JudgeExpectation {
    pub fn matches(self, observed: JudgeObserved) -> bool {
        matches!(
            (self, observed),
            (JudgeExpectation::Pass, JudgeObserved::Pass)
                | (JudgeExpectation::Fail, JudgeObserved::Fail)
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            JudgeExpectation::Pass => "pass",
            JudgeExpectation::Fail => "fail",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeObserved {
    Pass,
    Fail,
}

impl JudgeObserved {
    pub fn as_str(self) -> &'static str {
        match self {
            JudgeObserved::Pass => "pass",
            JudgeObserved::Fail => "fail",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeKind {
    Validator,
    Checker,
}

impl JudgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            JudgeKind::Validator => "validator",
            JudgeKind::Checker => "checker",
        }
    }
}

#[derive(Clone, Debug)]
pub struct JudgeValidatorOptions {
    pub work_dir: PathBuf,
    pub validator: Option<String>,
    pub input_path: PathBuf,
    pub expect: JudgeExpectation,
    pub output_limit_bytes: usize,
    pub fix_line_endings: bool,
    pub line_ending_hints: bool,
}

#[derive(Clone, Debug)]
pub struct JudgeCheckerOptions {
    pub work_dir: PathBuf,
    pub checker: Option<String>,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub answer_path: PathBuf,
    pub expect: JudgeExpectation,
    pub output_limit_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct JudgeReport {
    pub ok: bool,
    pub expect: JudgeExpectation,
    pub observed: JudgeObserved,
    pub kind: JudgeKind,
    pub program: String,
    pub run: RunResult,
    pub report_path: Option<PathBuf>,
    pub report: Option<String>,
    pub warnings: Vec<JudgeWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct JudgeWarning {
    pub code: &'static str,
    pub message: String,
}

impl JudgeReport {
    pub fn summary_line(&self) -> String {
        let status = if self.ok { "ok" } else { "mismatch" };
        format!(
            "judge {}: {} program={} expect={} observed={} {}",
            self.kind.as_str(),
            status,
            self.program,
            self.expect.as_str(),
            self.observed.as_str(),
            self.run.status_line()
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CheckerRun {
    pub(crate) checker: String,
    pub(crate) result: RunResult,
    pub(crate) report: Option<String>,
    pub(crate) report_path: Option<PathBuf>,
}

pub fn judge_validator(options: JudgeValidatorOptions) -> Result<JudgeReport> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let problem = load_problem(&work_dir)?;
    let validator_name = options
        .validator
        .or(problem.validator_name.clone())
        .context("validator is not configured; pass --validator or set problem.yaml validator")?;
    let validator = resolve_named_or_source(&work_dir, &problem, &validator_name)?;
    let input_path = resolve_path(&work_dir, &options.input_path);
    let mut input = std::fs::read(&input_path)
        .with_context(|| format!("failed to read input {}", input_path.display()))?;
    let normalization_warnings = if options.fix_line_endings {
        fix_validator_input_line_endings(&mut input)
    } else {
        Vec::new()
    };
    if !normalization_warnings.is_empty() {
        std::fs::write(&input_path, &input).with_context(|| {
            format!(
                "failed to normalize validator input {}",
                input_path.display()
            )
        })?;
    }
    let warnings = if options.line_ending_hints {
        normalization_warnings
    } else {
        Vec::new()
    };
    let result = run_spec(
        &work_dir,
        &validator,
        &[],
        Some(&input),
        normalized_output_limit(options.output_limit_bytes),
    )?;
    Ok(report_from_result(
        JudgeKind::Validator,
        validator_name,
        options.expect,
        result,
        None,
        None,
        warnings,
    ))
}

pub fn judge_checker(options: JudgeCheckerOptions) -> Result<JudgeReport> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let problem = load_problem(&work_dir)?;
    let checker_name = options
        .checker
        .or(problem.checker_name.clone())
        .context("checker is not configured; pass --checker or set problem.yaml checker")?;
    let checker = resolve_named_or_source(&work_dir, &problem, &checker_name)?;
    let input_path = resolve_path(&work_dir, &options.input_path);
    let output_path = resolve_path(&work_dir, &options.output_path);
    let answer_path = resolve_path(&work_dir, &options.answer_path);
    let report_path = persistent_report_path(&work_dir)?;
    let run = run_checker_files(
        &work_dir,
        &checker,
        &input_path,
        &output_path,
        &answer_path,
        &report_path,
        normalized_output_limit(options.output_limit_bytes),
    )?;
    Ok(report_from_result(
        JudgeKind::Checker,
        checker_name,
        options.expect,
        run.result,
        run.report_path,
        run.report,
        Vec::new(),
    ))
}

pub(crate) fn run_configured_checker_on_bytes(
    work_dir: &Path,
    problem: &super::schema::Problem,
    input: &[u8],
    output: &[u8],
    answer: &[u8],
    output_limit_bytes: usize,
) -> Result<Option<CheckerRun>> {
    let Some(checker_name) = &problem.checker_name else {
        return Ok(None);
    };
    let checker = resolve_named_or_source(work_dir, problem, checker_name)?;
    Ok(Some(run_checker_bytes(
        work_dir,
        checker_name,
        &checker,
        input,
        output,
        answer,
        output_limit_bytes,
    )?))
}

pub(crate) fn run_configured_checker_on_files(
    work_dir: &Path,
    problem: &super::schema::Problem,
    input_path: &Path,
    output_path: &Path,
    answer_path: &Path,
    report_path: &Path,
    output_limit_bytes: usize,
) -> Result<Option<CheckerRun>> {
    let Some(checker_name) = &problem.checker_name else {
        return Ok(None);
    };
    let checker = resolve_named_or_source(work_dir, problem, checker_name)?;
    let mut run = run_checker_files(
        work_dir,
        &checker,
        input_path,
        output_path,
        answer_path,
        report_path,
        output_limit_bytes,
    )?;
    run.checker = checker_name.clone();
    Ok(Some(run))
}

fn report_from_result(
    kind: JudgeKind,
    program: String,
    expect: JudgeExpectation,
    mut run: RunResult,
    report_path: Option<PathBuf>,
    report: Option<String>,
    warnings: Vec<JudgeWarning>,
) -> JudgeReport {
    run.set_phase(kind.as_str());
    if !run.ok
        && matches!(run.exit_code, Some(1 | 2))
        && !run.truncated_stdout
        && !run.truncated_stderr
    {
        let reason = match kind {
            JudgeKind::Validator => "validator_rejected",
            JudgeKind::Checker => "checker_rejected",
        };
        run.set_verdict("WA", reason);
    }
    let observed = if run.ok {
        JudgeObserved::Pass
    } else {
        JudgeObserved::Fail
    };
    JudgeReport {
        ok: expect.matches(observed),
        expect,
        observed,
        kind,
        program,
        run,
        report_path,
        report,
        warnings,
    }
}

pub(crate) fn fix_validator_input_line_endings(input: &mut Vec<u8>) -> Vec<JudgeWarning> {
    let sample = line_ending_sample(input, 8);
    if !sample.needs_normalization() {
        return Vec::new();
    }

    let mut normalized = Vec::with_capacity(input.len() + native_newline().len());
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'\r' => {
                if input.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                normalized.extend_from_slice(native_newline());
            }
            b'\n' => normalized.extend_from_slice(native_newline()),
            byte => normalized.push(byte),
        }
        index += 1;
    }
    if sample.missing_final_eol && !normalized.is_empty() {
        normalized.extend_from_slice(native_newline());
    }
    *input = normalized;

    vec![JudgeWarning {
        code: "input_line_endings_normalized",
        message: format!(
            "normalized validator input line endings before running; sampled head/tail lines found crlf={}, lone_lf={}, lone_cr={}, missing_final_eol={}",
            sample.crlf, sample.lone_lf, sample.lone_cr, sample.missing_final_eol
        ),
    }]
}

#[derive(Default)]
struct LineEndingSample {
    crlf: usize,
    lone_lf: usize,
    lone_cr: usize,
    missing_final_eol: bool,
}

impl LineEndingSample {
    fn needs_normalization(&self) -> bool {
        let non_native = if cfg!(windows) {
            self.lone_lf + self.lone_cr
        } else {
            self.crlf + self.lone_cr
        };
        non_native > 0 || self.missing_final_eol
    }
}

fn line_ending_sample(input: &[u8], lines: usize) -> LineEndingSample {
    let mut endings = Vec::new();
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'\r' if input.get(index + 1) == Some(&b'\n') => {
                endings.push(LineEndingKind::Crlf);
                index += 2;
            }
            b'\r' => {
                endings.push(LineEndingKind::LoneCr);
                index += 1;
            }
            b'\n' => {
                endings.push(LineEndingKind::LoneLf);
                index += 1;
            }
            _ => index += 1,
        }
    }

    let mut sample = LineEndingSample {
        missing_final_eol: !input.is_empty() && !matches!(input.last(), Some(b'\n' | b'\r')),
        ..LineEndingSample::default()
    };
    let tail_start = endings.len().saturating_sub(lines);
    for (idx, ending) in endings.iter().enumerate() {
        if idx >= lines && idx < tail_start {
            continue;
        }
        match ending {
            LineEndingKind::Crlf => sample.crlf += 1,
            LineEndingKind::LoneLf => sample.lone_lf += 1,
            LineEndingKind::LoneCr => sample.lone_cr += 1,
        }
    }
    sample
}

#[derive(Clone, Copy)]
enum LineEndingKind {
    Crlf,
    LoneLf,
    LoneCr,
}

fn native_newline() -> &'static [u8] {
    if cfg!(windows) { b"\r\n" } else { b"\n" }
}

fn run_checker_bytes(
    work_dir: &Path,
    checker_name: &str,
    checker: &ProgramSpec,
    input: &[u8],
    output: &[u8],
    answer: &[u8],
    output_limit_bytes: usize,
) -> Result<CheckerRun> {
    let temp_dir = work_dir
        .join(".cptool")
        .join("tmp")
        .join(format!("checker-{}", temp_suffix()));
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("failed to create checker temp dir {}", temp_dir.display()))?;
    let input_path = temp_dir.join("input.txt");
    let output_path = temp_dir.join("output.txt");
    let answer_path = temp_dir.join("answer.txt");
    let report_path = temp_dir.join("report.txt");
    std::fs::write(&input_path, input)
        .with_context(|| format!("failed to write {}", input_path.display()))?;
    std::fs::write(&output_path, output)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    std::fs::write(&answer_path, answer)
        .with_context(|| format!("failed to write {}", answer_path.display()))?;
    let mut run = run_checker_files(
        work_dir,
        checker,
        &input_path,
        &output_path,
        &answer_path,
        &report_path,
        output_limit_bytes,
    )?;
    run.checker = checker_name.to_string();
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(run)
}

fn run_checker_files(
    work_dir: &Path,
    checker: &ProgramSpec,
    input_path: &Path,
    output_path: &Path,
    answer_path: &Path,
    report_path: &Path,
    output_limit_bytes: usize,
) -> Result<CheckerRun> {
    let report_parent = report_path
        .parent()
        .context("checker report path has no parent")?;
    std::fs::create_dir_all(report_parent).with_context(|| {
        format!(
            "failed to create checker report parent {}",
            report_parent.display()
        )
    })?;
    let args = vec![
        input_path.to_string_lossy().into_owned(),
        output_path.to_string_lossy().into_owned(),
        answer_path.to_string_lossy().into_owned(),
        report_path.to_string_lossy().into_owned(),
    ];
    let result = run_spec(work_dir, checker, &args, None, output_limit_bytes)?;
    let report = std::fs::read_to_string(report_path).ok();
    let report_path = report.as_ref().map(|_| report_path.to_path_buf());
    Ok(CheckerRun {
        checker: checker.label.clone(),
        result,
        report,
        report_path,
    })
}

fn persistent_report_path(work_dir: &Path) -> Result<PathBuf> {
    let dir = work_dir.join(".cptool").join("tmp");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create judge temp dir {}", dir.display()))?;
    Ok(dir.join(format!("judge-{}.txt", temp_suffix())))
}

fn normalized_output_limit(output_limit_bytes: usize) -> usize {
    if output_limit_bytes == 0 {
        DEFAULT_OUTPUT_LIMIT_BYTES
    } else {
        output_limit_bytes
    }
}
