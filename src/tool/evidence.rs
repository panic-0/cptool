use super::check::{CheckIssue, CheckOptions, CheckReport, check_problem_package_with_options};
use super::data::{GenerateOptions, GenerateReport, generate_data_report_with_options};
use super::schema::DEFAULT_OUTPUT_LIMIT_BYTES;
use super::stress::StressSummary;
use super::stress_plan::{StressPlanOptions, stress_plan_collect_with_options};
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
    pub skip_stress_plan: bool,
    pub reuse_existing_stress_plan: Option<PathBuf>,
    pub generation_lock_timeout: Option<Duration>,
}

impl EvidenceOptions {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            work_dir,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            skip_gen: false,
            skip_stress_plan: false,
            reuse_existing_stress_plan: None,
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
    pub stress_plan: EvidenceSection<Vec<StressSummary>>,
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
            || self.stress_plan.is_error()
    }

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# cptool evidence report\n\n");
        out.push_str(&format!("- cptool_version: `{}`\n", self.cptool_version));
        out.push_str(&format!("- work_dir: `{}`\n", self.work_dir.display()));
        out.push_str(&format!("- check: {}\n", self.check.summary()));
        out.push_str(&format!("- gen: {}\n", self.r#gen.summary()));
        out.push_str(&format!("- stress_plan: {}\n", self.stress_plan.summary()));
        out
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
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
    pub status: EvidenceStatus,
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
                format!("ok plans={} cases={}", report.len(), cases)
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
        skip_stress_plan,
        reuse_existing_stress_plan,
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
    let stress_plan = if skip_stress_plan {
        EvidenceSection::skipped("requested_by_user")
    } else if let Some(path) = reuse_existing_stress_plan {
        collect_reused_stress_plan(&path)
    } else {
        collect_stress_plan(&work_dir, output_limit_bytes, generation_lock_timeout)
    };

    EvidenceReport {
        cptool_version: env!("CPTOOL_VERSION").to_string(),
        work_dir,
        check,
        r#gen,
        stress_plan,
    }
}

#[derive(Deserialize)]
struct StressPlanJsonReport {
    plans: Vec<StressSummary>,
}

fn collect_reused_stress_plan(path: &Path) -> EvidenceSection<Vec<StressSummary>> {
    match read_reused_stress_plan(path) {
        Ok(report) => EvidenceSection::ok(report),
        Err(err) => EvidenceSection::error(err.to_string()),
    }
}

fn read_reused_stress_plan(path: &Path) -> anyhow::Result<Vec<StressSummary>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read stress-plan JSON `{}`", path.display()))?;
    let report: StressPlanJsonReport = serde_json::from_str(&text).with_context(|| {
        format!(
            "failed to parse stress-plan JSON `{}`; expected output from `cptool stress-plan --summary-only --json`",
            path.display()
        )
    })?;
    Ok(report.plans)
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
        clean: false,
        generation_lock_timeout,
    }) {
        Ok(report) => EvidenceSection::ok(report),
        Err(err) => EvidenceSection::error(err.to_string()),
    }
}

fn collect_stress_plan(
    work_dir: &Path,
    output_limit_bytes: usize,
    generation_lock_timeout: Option<Duration>,
) -> EvidenceSection<Vec<StressSummary>> {
    match stress_plan_collect_with_options(StressPlanOptions {
        work_dir,
        name: None,
        failure_dir: None,
        output_limit_bytes,
        summary_only: true,
        filter: super::stress_plan::StressPlanFilter::All,
        generation_lock_timeout,
    }) {
        Ok(report) => EvidenceSection::ok(report),
        Err(err) => EvidenceSection::error(err.to_string()),
    }
}
