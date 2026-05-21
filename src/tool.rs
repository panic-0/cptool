mod check;
mod clean;
mod data;
mod evidence;
mod package;
mod problem;
mod program;
mod run;
mod schema;
mod stress;
mod stress_args;
mod stress_plan;

pub(crate) use crate::support::{temp_suffix, unix_epoch_nanos};
pub use check::{
    CheckIssue, CheckOptions, CheckReport, CheckSeverity, check_problem_package,
    check_problem_package_with_options,
};
pub use clean::{CleanOptions, CleanReport, clean_package_with_options};
pub use data::{
    GenerateOptions, GenerateReport, GenerateWarning, GenerateWarningKind, generate_data,
    generate_data_report_with_options, generate_data_with_options,
};
pub use evidence::{
    EvidenceCheckReport, EvidenceOptions, EvidenceReport, EvidenceSection, collect_evidence,
};
pub use package::{init_package, slugify};
pub(crate) use problem::resolve_path;
pub use problem::{load_problem, parse_case_selector};
pub use run::run;
pub use schema::{
    CommandProgram, CppProgram, DEFAULT_OUTPUT_LIMIT_BYTES, OutputConfig, Problem, Program,
    ProgramInfo, RunOptions, RunResult, Stress, StressPlan, StressPlanExpectation, Test,
    TestBundle, TestCase, TestTask, TestTaskType,
};
pub use stress::{
    ExpectedStressFailure, StressOptions, StressSummary, StressWarning, stress,
    stress_with_options, stress_with_summary,
};
pub use stress_plan::{
    StressPlanFilter, StressPlanOptions, stress_plan, stress_plan_collect_with_options,
    stress_plan_with_options,
};

#[cfg(test)]
pub(crate) fn temp_test_dir(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}", temp_suffix()))
}

#[cfg(test)]
mod tests {
    use super::stress::{classify_stress_failure, normalize_output};
    use super::*;

    #[test]
    fn slugify_keeps_ascii_ids_predictable() {
        assert_eq!(slugify("My Problem 01").unwrap(), "my-problem-01");
        assert_eq!(slugify(" already_ok ").unwrap(), "already_ok");
        assert!(slugify("   ").is_err());
    }

    #[test]
    fn parse_case_selector_uses_zero_based_index() {
        let selector = parse_case_selector("s1[0]").unwrap();
        assert_eq!(selector.bundle, "s1");
        assert_eq!(selector.index, 0);
        assert!(parse_case_selector("s1").is_err());
        assert!(parse_case_selector("[0]").is_err());
    }

    #[test]
    fn normalize_output_trims_trailing_space_and_final_blankness() {
        assert_eq!(normalize_output("1  \r\n2\n\n"), "1\n2\n");
        assert_eq!(normalize_output("  \n"), "");
    }

    #[test]
    fn init_package_creates_cptool_layout() {
        let root = temp_test_dir("cptool-test");
        let problem_dir = init_package(&root, "My Problem").unwrap();
        assert_eq!(problem_dir.file_name().unwrap(), "my-problem");
        assert!(problem_dir.join("problem.yaml").exists());
        assert!(problem_dir.join("src").join("std.cpp").exists());
        assert!(problem_dir.join("src").join("brute.cpp").exists());
        assert!(problem_dir.join("src").join("gen.cpp").exists());
        assert!(problem_dir.join("tests").join("failures").is_dir());
        assert!(problem_dir.join(".gitignore").exists());
        assert!(!problem_dir.join("quality_report.md").exists());
        assert!(!problem_dir.join("problem.md").exists());

        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.programs["gen"].time_limit_secs, 3.0);
        assert_eq!(problem.programs["std"].time_limit_secs, 3.0);
        assert_eq!(problem.programs["brute"].time_limit_secs, 3.0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn init_package_writes_loadable_yaml_for_special_problem_names() {
        let root = temp_test_dir("cptool-yaml-name-test");

        let problem_dir = init_package(&root, "My Problem: #1").unwrap();
        let problem = load_problem(&problem_dir).unwrap();

        assert_eq!(problem_dir.file_name().unwrap(), "my-problem-1");
        assert_eq!(problem.name, "My Problem: #1");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn run_result_reports_timeout_without_stderr() {
        let result = RunResult {
            label: "slow".to_string(),
            ok: false,
            kind: "timeout".to_string(),
            exit_code: None,
            diagnostic: None,
            elapsed_ms: 1001,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            truncated_stdout: false,
            truncated_stderr: false,
        };

        assert_eq!(
            result.status_line(),
            "slow: timeout exit=none elapsed=1001ms"
        );
        assert_eq!(
            result.failure_report("generator failed"),
            "generator failed: slow: timeout exit=none elapsed=1001ms"
        );
    }

    #[test]
    fn stress_failure_classification_names_wa_and_program_failure() {
        let ok_a = test_run_result("std", true, "ok", "1\n", "");
        let ok_b = test_run_result("brute", true, "ok", "2\n", "");
        let timeout = test_run_result("slow", false, "timeout", "", "");

        assert_eq!(
            classify_stress_failure(&[ok_a.clone(), ok_b]).unwrap(),
            "wrong_answer: output mismatch between `std` and `brute`"
        );
        assert_eq!(
            classify_stress_failure(&[ok_a, timeout]).unwrap(),
            "program_failed: slow: timeout exit=none elapsed=1ms"
        );
    }

    fn test_run_result(label: &str, ok: bool, kind: &str, stdout: &str, stderr: &str) -> RunResult {
        RunResult {
            label: label.to_string(),
            ok,
            kind: kind.to_string(),
            exit_code: None,
            diagnostic: None,
            elapsed_ms: 1,
            stdout_bytes: stdout.as_bytes().to_vec(),
            stderr_bytes: stderr.as_bytes().to_vec(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            truncated_stdout: false,
            truncated_stderr: false,
        }
    }
}
