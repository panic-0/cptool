use serde::Serialize;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CheckIssue {
    pub severity: CheckSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transient: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    pub work_dir: PathBuf,
    pub issues: Vec<CheckIssue>,
}

#[derive(Clone, Debug, Default)]
pub struct CheckOptions {
    pub generation_lock_timeout: Option<Duration>,
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
            kind: None,
            transient: None,
            retry_after: None,
            location: None,
        });
    }

    fn push_at(
        &mut self,
        severity: CheckSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
        location: impl Into<String>,
    ) {
        self.issues.push(CheckIssue {
            severity,
            code: code.into(),
            message: message.into(),
            path,
            kind: None,
            transient: None,
            retry_after: None,
            location: Some(location.into()),
        });
    }

    pub(super) fn lock_error(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) {
        self.issues.push(CheckIssue {
            severity: CheckSeverity::Error,
            code: code.into(),
            message: message.into(),
            path,
            kind: Some("lock".to_string()),
            transient: Some(true),
            retry_after: Some("wait_for_generation_then_retry".to_string()),
            location: None,
        });
    }

    pub(super) fn error(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) {
        self.push(CheckSeverity::Error, code, message, path);
    }

    pub(super) fn warning(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) {
        self.push(CheckSeverity::Warning, code, message, path);
    }

    pub(super) fn error_at(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
        location: impl Into<String>,
    ) {
        self.push_at(CheckSeverity::Error, code, message, path, location);
    }

    pub(super) fn warning_at(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<PathBuf>,
        location: impl Into<String>,
    ) {
        self.push_at(CheckSeverity::Warning, code, message, path, location);
    }
}
