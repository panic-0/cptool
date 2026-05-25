mod add;
mod batch_args;
mod check;
mod clean;
mod data;
mod evidence;
mod explain;
mod fixture;
mod judge;
mod package;
mod problem;
mod program;
mod run;
mod schema;
mod stress;
mod task_expect;

pub(crate) use crate::support::{temp_suffix, unix_epoch_nanos};
pub use add::{
    AddBundleOptions, AddCheckerOptions, AddProgramKind, AddProgramOptions, AddTaskOptions,
    AddValidatorOptions, add_bundle, add_checker, add_program, add_task, add_validator,
};
pub use batch_args::range_args;
pub use check::{
    CheckIssue, CheckIssueDetail, CheckOptions, CheckReport, CheckSeverity,
    check_problem_package_with_options,
};
pub use clean::{CleanOptions, CleanReport, clean_package_with_options};
pub use data::{
    GenerateOptions, GenerateReport, GenerateWarning, GenerateWarningKind,
    generate_data_report_with_options, generate_data_with_options,
};
pub use evidence::{
    EvidenceCheckReport, EvidenceOptions, EvidenceReport, EvidenceSection, collect_evidence,
};
pub use explain::{ExplainOptions, ExplainProgramRef, ExplainReport, explain_package};
pub use fixture::{
    AddCheckerFixtureOptions, AddFixtureReport, AddInputFixtureOptions, AddValidatorFixtureOptions,
    CheckerFixture, FixtureCheckReport, FixtureIssue, FixtureListReport, InputFixture,
    ValidatorFixture, add_checker_fixture, add_input_fixture, add_validator_fixture,
    check_fixtures, checker_fixture_reports, list_fixtures, validator_fixture_reports,
};
pub use judge::{
    JudgeCheckerOptions, JudgeExpectation, JudgeKind, JudgeObserved, JudgeReport,
    JudgeValidatorOptions, JudgeWarning, judge_checker, judge_validator,
};
pub use package::init_package;
pub use problem::load_problem;
pub(crate) use problem::resolve_path;
pub use run::run;
pub use schema::{
    CommandProgram, CompileFailure, CompileReport, CppProgram, DEFAULT_OUTPUT_LIMIT_BYTES,
    OutputConfig, Problem, Program, ProgramInfo, RunOptions, RunResult, Test, TestBundle, TestCase,
    TestTask, TestTaskType,
};
pub use stress::{
    ExpectedCheckerOutput, ExpectedStressFailure, ExpectedStressOutput, StressExpectOptions,
    StressSummary, StressWarning, stress_expect_with_options,
};
pub use task_expect::{
    TaskExpectOptions, task_expect_collect_with_options, task_expect_with_options,
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
        assert_eq!(package::slugify("My Problem 01").unwrap(), "my-problem-01");
        assert_eq!(package::slugify(" already_ok ").unwrap(), "already_ok");
        assert!(package::slugify("   ").is_err());
    }

    #[test]
    fn parse_case_selector_uses_zero_based_index() {
        let selector = problem::parse_case_selector("s1[0]").unwrap();
        assert_eq!(selector.bundle, "s1");
        assert_eq!(selector.index, 0);
        assert!(problem::parse_case_selector("s1").is_err());
        assert!(problem::parse_case_selector("[0]").is_err());
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
        assert!(problem_dir.join("src").join("val.cpp").exists());
        assert!(problem_dir.join("src").join("chk.cpp").exists());
        assert!(problem_dir.join("src").join("testlib.h").exists());
        let std_source = std::fs::read_to_string(problem_dir.join("src").join("std.cpp")).unwrap();
        let brute_source =
            std::fs::read_to_string(problem_dir.join("src").join("brute.cpp")).unwrap();
        assert_eq!(std_source, crate::tool::package::DEFAULT_PROGRAM_CPP);
        assert_eq!(brute_source, crate::tool::package::DEFAULT_PROGRAM_CPP);
        assert!(std_source.contains("#include <bits/stdc++.h>"));
        let normalized_std_source = std_source.replace("\r\n", "\n");
        assert!(
            normalized_std_source
                .contains("cin.tie(nullptr);\n    ios::sync_with_stdio(false);\n\n    return 0;")
        );
        let generator_source =
            std::fs::read_to_string(problem_dir.join("src").join("gen.cpp")).unwrap();
        assert!(generator_source.contains("#include \"testlib.h\""));
        assert!(generator_source.contains("registerGen(argc, argv, 1);"));
        let checker_source =
            std::fs::read_to_string(problem_dir.join("src").join("chk.cpp")).unwrap();
        assert!(checker_source.starts_with("// Copied from testlib checkers/wcmp.cpp\n"));
        assert!(checker_source.contains("compare sequences of tokens"));
        assert!(problem_dir.join("fixtures").join("input").is_dir());
        assert!(
            problem_dir
                .join("fixtures")
                .join("validator")
                .join("pass")
                .is_dir()
        );
        assert!(
            problem_dir
                .join("fixtures")
                .join("validator")
                .join("fail")
                .is_dir()
        );
        assert!(
            problem_dir
                .join("fixtures")
                .join("checker")
                .join("pass")
                .is_dir()
        );
        assert!(
            problem_dir
                .join("fixtures")
                .join("checker")
                .join("fail")
                .is_dir()
        );
        assert!(problem_dir.join(".cptool").join("failures").is_dir());
        assert!(problem_dir.join(".gitignore").exists());
        assert!(!problem_dir.join("quality_report.md").exists());
        assert!(!problem_dir.join("problem.md").exists());

        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.time_limit_secs, 3.0);
        assert_eq!(problem.memory_limit_mb, 512.0);
        assert_eq!(problem.cpp_compile_args, ["-O2", "-std=c++20"]);
        assert_eq!(problem.programs["gen"].time_limit_secs, 3.0);
        assert_eq!(problem.programs["std"].time_limit_secs, 3.0);
        assert_eq!(problem.programs["brute"].time_limit_secs, 3.0);
        assert_eq!(problem.programs["chk"].time_limit_secs, 3.0);
        let yaml = std::fs::read_to_string(problem_dir.join("problem.yaml")).unwrap();
        assert!(yaml.contains("time_limit_secs: 3.0\n"));
        assert!(yaml.contains("memory_limit_mb: 512.0\n"));
        assert!(yaml.contains("cpp_compile_args: [-O2, -std=c++20]\n"));
        assert!(!yaml.contains(
            "programs:\n  gen:\n    info: !cpp\n      path: ./src/gen.cpp\n    time_limit_secs"
        ));
        assert!(!yaml.contains("      compile_args:"));
        assert_eq!(problem.validator_name.as_deref(), Some("val"));
        assert_eq!(problem.checker_name.as_deref(), Some("chk"));
        match &problem.programs["val"].info {
            ProgramInfo::Cpp(program) => {
                assert_eq!(program.path, std::path::PathBuf::from("./src/val.cpp"))
            }
            other => panic!("expected val to be a C++ program, got {other:?}"),
        }
        match &problem.programs["chk"].info {
            ProgramInfo::Cpp(program) => {
                assert_eq!(program.path, std::path::PathBuf::from("./src/chk.cpp"))
            }
            other => panic!("expected chk to be a C++ program, got {other:?}"),
        }

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
            verdict: "TLE".to_string(),
            phase: "unknown".to_string(),
            reason_code: "timeout".to_string(),
            exit_code: None,
            diagnostic: None,
            elapsed_ms: 1001,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            truncated_stdout: false,
            truncated_stderr: false,
            compile: CompileReport::not_applicable(),
        };

        assert_eq!(
            result.result_line(),
            "slow: verdict=TLE phase=unknown reason=timeout exit=none elapsed=1001ms"
        );
        assert_eq!(
            result.failure_report("generator failed"),
            "generator failed: slow: verdict=TLE phase=unknown reason=timeout exit=none elapsed=1001ms"
        );
    }

    #[test]
    fn stress_failure_classification_names_wa_and_program_failure() {
        let ok_a = test_run_result("std", "AC", "1\n", "");
        let ok_b = test_run_result("brute", "AC", "2\n", "");
        let timeout = test_run_result("slow", "TLE", "", "");

        assert_eq!(
            classify_stress_failure(&[ok_a.clone(), ok_b]).unwrap(),
            "wrong_answer: output mismatch between `std` and `brute`"
        );
        assert_eq!(
            classify_stress_failure(&[ok_a, timeout]).unwrap(),
            "program_failed: slow: verdict=TLE phase=unknown reason=timeout exit=none elapsed=1ms"
        );
    }

    fn test_run_result(label: &str, verdict: &str, stdout: &str, stderr: &str) -> RunResult {
        RunResult {
            label: label.to_string(),
            verdict: verdict.to_string(),
            phase: "unknown".to_string(),
            reason_code: if verdict == "AC" {
                "ok"
            } else if verdict == "TLE" {
                "timeout"
            } else {
                "nonzero_exit"
            }
            .to_string(),
            exit_code: None,
            diagnostic: None,
            elapsed_ms: 1,
            stdout_bytes: stdout.as_bytes().to_vec(),
            stderr_bytes: stderr.as_bytes().to_vec(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            truncated_stdout: false,
            truncated_stderr: false,
            compile: CompileReport::not_applicable(),
        }
    }
}
