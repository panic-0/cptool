mod common;
use common::*;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

#[test]
fn stress_plan_runs_named_plan_without_seed_config() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan");
    run_cptool(["init", "stress_plan_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_plan_problem");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("plan `tiny` case 1 ok"));
    assert!(stdout.contains("plan `tiny` case 2 ok"));
    assert!(stdout.contains("stress plan `tiny` passed: 2 cases"));
}
#[test]
fn stress_plan_json_waits_for_generation_lock_and_stays_parseable() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-json-wait-lock");
    run_cptool(
        ["init", "stress_plan_json_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("stress_plan_json_wait_lock");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, Duration::from_millis(500));

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
            "--json",
            "--wait-for-generation-lock",
            "1",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    handle.join().unwrap();
    assert_eq!(value["plans"][0]["plan_name"], "tiny");
    assert_eq!(value["plans"][0]["cases"], 2);
    assert!(stderr.contains("waiting for data generation lock:"));
}
#[test]
fn stress_plan_summary_only_suppresses_case_progress() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-summary");
    run_cptool(["init", "stress_plan_summary", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_plan_summary");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("tiny: ok cases=2 unique_input_hashes=1 against=std,brute elapsed="));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stdout.contains("empty_stdout_cases=0"));
    assert!(stdout.contains("all_empty_stdout_cases=0"));
    assert!(stdout.contains("warnings=repeated_input:1"));
    assert!(!stdout.contains("plan `tiny` case 1 ok"));
    assert!(!stdout.contains("stress plan `tiny` passed"));
    assert!(output.stderr.is_empty());
}
#[test]
fn stress_plan_summary_only_json_prints_plan_summaries() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-json");
    run_cptool(["init", "stress_plan_json", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_plan_json");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
            "--json",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["plans"][0]["plan_name"], "tiny");
    assert_eq!(value["plans"][0]["cases"], 2);
    assert_eq!(value["plans"][0]["unique_input_hashes"], 1);
    assert_eq!(value["plans"][0]["warnings"][0]["code"], "repeated_input");
    assert!(!stdout.contains("plan `tiny` case 1 ok"));
    assert!(output.stderr.is_empty());
}
#[test]
fn stress_plan_can_filter_positive_and_negative_plans() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-filters");
    run_cptool(["init", "stress_plan_filters", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_plan_filters");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("bad.py"),
        r#"import sys

a, b = map(int, sys.stdin.read().split())
sys.stdout.buffer.write(f"{a + b + 1}\n".encode("ascii"))
"#,
    )
    .unwrap();
    append_mixed_stress_plans(&problem_dir);

    let positive = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--positive-only",
            "--json",
        ],
        None,
    );
    let positive_value: Value = serde_json::from_slice(&positive.stdout).unwrap();
    assert_eq!(positive_value["plans"].as_array().unwrap().len(), 1);
    assert_eq!(positive_value["plans"][0]["plan_name"], "tiny-pass");
    assert!(positive_value["plans"][0]["expected_failure"].is_null());

    let negative = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--negative-only",
            "--json",
        ],
        None,
    );
    let negative_value: Value = serde_json::from_slice(&negative.stdout).unwrap();
    assert_eq!(negative_value["plans"].as_array().unwrap().len(), 1);
    assert_eq!(negative_value["plans"][0]["plan_name"], "bad-is-detected");
    assert_eq!(
        negative_value["plans"][0]["expected_failure"]["failed_cases"],
        2
    );
}
#[test]
fn stress_warns_when_all_against_stdout_is_empty_on_non_empty_input() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-empty-output");
    run_cptool(["init", "stress_empty_output", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_empty_output");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let output = run_cptool(
        [
            "stress",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--against",
            "std",
            "--against",
            "brute",
            "--cases",
            "2",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("stress passed: 2 cases"));
    assert!(stderr.contains("warning: all_empty_output case=1 against=std,brute input_bytes=4"));
    assert!(stderr.contains("warning: all_empty_output case=2 against=std,brute input_bytes=4"));
    assert!(stderr.contains(
        "warning: repeated_input cases=2 unique_input_hashes=1 random_coverage=false hint=generator_args_produced_identical_inputs"
    ));
}
#[test]
fn stress_reports_single_unique_input_hash_for_fixed_args() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-fixed-args");
    run_cptool(["init", "stress_fixed_args", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_fixed_args");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "stress",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--against",
            "std",
            "--against",
            "brute",
            "--cases",
            "3",
            "--",
            "5",
            "8",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("stress passed: 3 cases"));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stderr.contains(
        "warning: repeated_input cases=3 unique_input_hashes=1 random_coverage=false hint=generator_args_produced_identical_inputs"
    ));
}
#[test]
fn stress_json_reports_unique_inputs_and_warnings_without_progress() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-json");
    run_cptool(["init", "stress_json", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_json");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "stress",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--against",
            "std",
            "--against",
            "brute",
            "--cases",
            "3",
            "--json",
            "--",
            "5",
            "8",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["cases"], 3);
    assert_eq!(value["unique_input_hashes"], 1);
    assert_eq!(value["warnings"][0]["code"], "repeated_input");
    assert_eq!(value["warnings"][0]["random_coverage"], false);
    assert!(!stdout.contains("case 1 ok"));
    assert!(output.stderr.is_empty());
}
#[test]
fn stress_expands_case_placeholder_and_reports_unique_inputs() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-case-placeholder");
    run_cptool(
        ["init", "stress_case_placeholder", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("stress_case_placeholder");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "stress",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--against",
            "std",
            "--against",
            "brute",
            "--cases",
            "3",
            "--",
            "{case}",
            "10",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("case 1 ok"));
    assert!(stdout.contains("case 2 ok"));
    assert!(stdout.contains("case 3 ok"));
    assert!(stdout.contains("unique_input_hashes=3"));
}
#[test]
fn stress_plan_summary_only_reports_empty_stdout_warning_count() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-empty-summary");
    run_cptool(
        ["init", "stress_plan_empty_summary", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("stress_plan_empty_summary");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("tiny: ok cases=2 unique_input_hashes=1 against=std,brute elapsed="));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stdout.contains("empty_stdout_cases=2"));
    assert!(stdout.contains("all_empty_stdout_cases=2"));
    assert!(stdout.contains("warnings=all_empty_output:2,repeated_input:1"));
    assert!(!stderr.contains("warning: all_empty_output"));
    assert!(!stderr.contains("warning: repeated_input"));
}
#[test]
fn stress_plan_expands_seed_and_case_placeholders() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-placeholders");
    run_cptool(
        ["init", "stress_plan_placeholders", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("stress_plan_placeholders");
    configure_python_problem(&problem_dir);
    overwrite_generator_for_stress_plan_placeholders(&problem_dir);
    append_stress_plan_with_seed_placeholders(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "seeded",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("plan `seeded` case 1 ok"));
    assert!(stdout.contains("plan `seeded` case 2 ok"));
    assert!(stdout.contains("stress plan `seeded` passed: 2 cases"));
}
#[test]
fn stress_plan_expect_fail_treats_wrong_answer_as_success() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-expect-fail");
    run_cptool(
        ["init", "stress_plan_expect_fail", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("stress_plan_expect_fail");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("bad.py"),
        r#"import sys

a, b = map(int, sys.stdin.read().split())
sys.stdout.buffer.write(f"{a + b + 1}\n".encode("ascii"))
"#,
    )
    .unwrap();
    append_expect_fail_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "bad-is-detected",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("bad-is-detected: expected_fail observed=true case=1"));
    assert!(stdout.contains("reason=wrong_answer: output mismatch between `std` and `bad`"));
    assert!(stdout.contains("failed_cases=3"));
    assert!(stdout.contains("passed_cases=0"));
    assert!(stdout.contains("failure_ratio=1.000"));
    assert!(stdout.contains("cases_run=3"));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(
        problem_dir
            .join("tests")
            .join("failures")
            .join("stress-bad-is-detected-001.txt")
            .exists()
    );

    let json_output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "bad-is-detected",
            "--summary-only",
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    let failure = &value["plans"][0]["expected_failure"];

    assert_eq!(value["plans"][0]["cases"], 3);
    assert_eq!(value["plans"][0]["unique_input_hashes"], 1);
    assert_eq!(failure["failed_cases"], 3);
    assert_eq!(failure["passed_cases"], 0);
    assert_eq!(failure["failure_ratio"], 1.0);
    assert!(failure["input_sha256"].as_str().unwrap().len() == 64);
    assert!(Path::new(failure["input_path"].as_str().unwrap()).exists());
    assert!(Path::new(failure["report_path"].as_str().unwrap()).exists());
    assert_eq!(failure["outputs"].as_array().unwrap().len(), 2);
}
