mod data;
mod package;
mod problem;
mod program;
mod run;
mod schema;
mod stress;

pub use data::generate_data;
pub use package::{init_package, slugify};
pub use problem::{load_problem, parse_case_selector};
pub use run::run;
pub use schema::{
    CommandProgram, CppProgram, Problem, Program, ProgramInfo, RunOptions, RunResult, Test,
    TestBundle, TestCase, TestTask, TestTaskType,
};
pub use stress::stress;
#[cfg(test)]
mod tests {
    use super::program::{is_stale_compile_lock, parse_lock_pid};
    use super::stress::normalize_output;
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
        let root = std::env::temp_dir().join(format!(
            "cptool-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parse_compile_lock_pid_reads_lock_file() {
        assert_eq!(parse_lock_pid("pid=123\n"), Some(123));
        assert_eq!(parse_lock_pid("owner=abc\npid=456\n"), Some(456));
        assert_eq!(parse_lock_pid("pid=not-a-number\n"), None);
    }

    #[test]
    fn stale_compile_lock_detects_dead_process() {
        let root = std::env::temp_dir().join(format!(
            "cptool-lock-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let lock_path = root.join("compile.lock");
        std::fs::write(&lock_path, "pid=999999\n").unwrap();

        assert!(is_stale_compile_lock(&lock_path).unwrap());

        std::fs::remove_dir_all(root).unwrap();
    }
}
