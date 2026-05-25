mod common;
use common::*;
use serde_json::Value;

#[test]
fn stress_plan_runs_named_plan() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan");
    run_cptool(
        ["pkg", "init", "stress_plan_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_problem");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("task `tiny:pass:brute` case 1 ok"));
    assert!(stdout.contains("task `tiny:pass:brute` case 2 ok"));
    assert!(stdout.contains("expect task `tiny` passed: 1 checks"));
}

#[test]
fn stress_uses_configured_checker_instead_of_text_comparison() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-checker");
    run_cptool(
        ["pkg", "init", "stress_checker", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_checker");
    configure_checker_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "batch",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--pass",
            "alt",
            "--json",
            "--",
            "7",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(value["checks"][0]["checker"], "chk");
    assert_eq!(value["checks"][0]["answer_program"], "std");
    assert_eq!(value["checks"][0]["expected_failure"], Value::Null);
}

#[test]
fn stress_plan_expect_fail_records_checker_rejection_artifact() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-checker-fail");
    run_cptool(
        ["pkg", "init", "stress_checker_fail", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_checker_fail");
    configure_checker_python_problem(&problem_dir);
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml.push_str(
        r#"stress:
  plans:
  - name: checker-catches-bad
    generator: gen
    args: ["7"]
    against: [std, bad]
    cases: 1
    expect: fail
"#,
    );
    std::fs::write(&yaml_path, yaml).unwrap();

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let plan = value["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["task_name"] == "checker-catches-bad:fail:bad")
        .unwrap();
    let failure = &plan["expected_failure"];

    assert_eq!(plan["checker"], "chk");
    assert_eq!(plan["answer_program"], "std");
    assert!(
        failure["reason"]
            .as_str()
            .unwrap()
            .contains("checker `chk` rejected")
    );
    assert_eq!(failure["checker"]["checker"], "chk");
    assert_eq!(failure["checker"]["participant"], "bad");
    let report_path = problem_dir.join(failure["checker"]["report_path"].as_str().unwrap());
    assert!(report_path.exists());
    assert!(
        std::fs::read_to_string(report_path)
            .unwrap()
            .contains("expected 7")
    );
}

#[test]
fn stress_plan_expect_fail_rejects_checker_infrastructure_failure() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-checker-crash");
    run_cptool(
        ["pkg", "init", "stress_checker_crash", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_checker_crash");
    configure_checker_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("chk.py"),
        r#"import sys
sys.stderr.write("checker crashed\n")
raise SystemExit(3)
"#,
    )
    .unwrap();
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml.push_str(
        r#"stress:
  plans:
  - name: checker-crash-is-not-wrong-answer
    generator: gen
    args: ["7"]
    against: [std, bad]
    cases: 1
    expect: fail
"#,
    );
    std::fs::write(&yaml_path, yaml).unwrap();

    let output = run_cptool_allow_failure(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("checker_failed"), "{stderr}");
    let report = std::fs::read_to_string(
        std::fs::read_dir(problem_dir.join(".cptool").join("failures"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|extension| extension == "txt"))
            .unwrap(),
    )
    .unwrap();
    assert!(report.contains("reason: checker_failed"), "{report}");
}

#[test]
fn stress_plan_json_waits_for_generation_lock_and_stays_parseable() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-json-wait-lock");
    run_cptool(
        ["pkg", "init", "stress_plan_json_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_json_wait_lock");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, GENERATION_LOCK_RELEASE_DELAY);

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
            "--json",
            "--wait-for-generation-lock",
            GENERATION_LOCK_WAIT_TIMEOUT_SECS,
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    handle.join().unwrap();
    assert_eq!(value["tasks"][0]["task_name"], "tiny:pass:brute");
    assert_eq!(value["tasks"][0]["cases"], 2);
    assert!(stderr.contains("waiting for data generation lock:"));
}
#[test]
fn stress_plan_summary_only_suppresses_case_progress() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-summary");
    run_cptool(
        ["pkg", "init", "stress_plan_summary", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_summary");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains(
            "tiny:pass:brute: ok cases=2 unique_input_hashes=1 against=std,brute elapsed="
        )
    );
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stdout.contains("empty_stdout_cases=0"));
    assert!(stdout.contains("all_empty_stdout_cases=0"));
    assert!(stdout.contains("warnings=repeated_input:1"));
    assert!(!stdout.contains("task `tiny:pass:brute` case 1 ok"));
    assert!(!stdout.contains("expect task `tiny` passed"));
    assert!(output.stderr.is_empty());
}
#[test]
fn stress_plan_summary_only_json_prints_plan_summaries() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-json");
    run_cptool(
        ["pkg", "init", "stress_plan_json", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_json");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "task",
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

    assert_eq!(value["tasks"][0]["task_name"], "tiny:pass:brute");
    assert_eq!(value["tasks"][0]["cases"], 2);
    assert_eq!(value["tasks"][0]["unique_input_hashes"], 1);
    assert_eq!(value["tasks"][0]["warnings"][0]["code"], "repeated_input");
    assert!(!stdout.contains("task `tiny` case 1 ok"));
    assert!(output.stderr.is_empty());
}
#[test]
fn test_task_runs_positive_and_negative_checks_together() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-filters");
    run_cptool(
        ["pkg", "init", "stress_plan_filters", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_filters");
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

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["tasks"].as_array().unwrap().len(), 3);
    assert!(value["tasks"].as_array().unwrap().iter().any(|plan| {
        plan["task_name"] == "tiny-pass:pass:brute" && plan["expected_failure"].is_null()
    }));
    let negative = value["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["task_name"] == "bad-is-detected:fail:bad")
        .unwrap();
    assert_eq!(negative["expected_failure"]["failed_cases"], 2);
}
#[test]
fn stress_warns_when_all_against_stdout_is_empty_on_non_empty_input() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-empty-output");
    run_cptool(
        ["pkg", "init", "stress_empty_output", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_empty_output");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let output = run_cptool(
        [
            "test",
            "batch",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--pass",
            "brute",
            "--",
            "{1:2}",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("batch expect passed: 2 cases"));
    assert!(stderr.contains("warning: all_empty_output case=1 against=std,brute input_bytes=4"));
    assert!(stderr.contains("warning: all_empty_output case=2 against=std,brute input_bytes=4"));
}
#[test]
fn stress_reports_unique_input_hashes_for_range_args() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-fixed-args");
    run_cptool(
        ["pkg", "init", "stress_fixed_args", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_fixed_args");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "batch",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--pass",
            "brute",
            "--",
            "{5:7}",
            "8",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("batch expect passed: 3 cases"));
    assert!(stderr.is_empty());
}
#[test]
fn stress_json_reports_unique_inputs_and_warnings_without_progress() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-json");
    run_cptool(["pkg", "init", "stress_json", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("stress_json");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "batch",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--pass",
            "brute",
            "--json",
            "--",
            "{5:7}",
            "8",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["checks"][0]["cases"], 3);
    assert_eq!(value["checks"][0]["unique_input_hashes"], 3);
    assert_eq!(value["checks"][0]["warnings"], Value::Array(Vec::new()));
    assert!(!stdout.contains("case 1 ok"));
    assert!(output.stderr.is_empty());
}
#[test]
fn stress_expands_range_and_reports_unique_inputs() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-case-placeholder");
    run_cptool(
        ["pkg", "init", "stress_case_placeholder", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_case_placeholder");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "batch",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--pass",
            "brute",
            "--",
            "{1:3}",
            "10",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("case 1 ok"));
    assert!(stdout.contains("case 2 ok"));
    assert!(stdout.contains("case 3 ok"));
    assert!(stdout.contains("batch expect passed: 3 cases"));
}
#[test]
fn stress_plan_summary_only_reports_empty_stdout_warning_count() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-empty-summary");
    run_cptool(
        ["pkg", "init", "stress_plan_empty_summary", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_empty_summary");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let output = run_cptool(
        [
            "test",
            "task",
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

    assert!(
        stdout.contains(
            "tiny:pass:brute: ok cases=2 unique_input_hashes=1 against=std,brute elapsed="
        )
    );
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stdout.contains("empty_stdout_cases=2"));
    assert!(stdout.contains("all_empty_stdout_cases=2"));
    assert!(stdout.contains("warnings=all_empty_output:2,repeated_input:1"));
    assert!(!stderr.contains("warning: all_empty_output"));
    assert!(!stderr.contains("warning: repeated_input"));
}
#[test]
fn stress_plan_expands_range_args() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-range");
    run_cptool(
        ["pkg", "init", "stress_plan_range", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_range");
    configure_python_problem(&problem_dir);
    overwrite_generator_for_range_args(&problem_dir);
    append_expect_task_with_range_args(&problem_dir);

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "range-proof",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("task `range-proof:pass:brute` case 1 ok"));
    assert!(stdout.contains("task `range-proof:pass:brute` case 2 ok"));
    assert!(stdout.contains("expect task `range-proof` passed: 1 checks"));
}

#[test]
fn stress_plan_accepts_inline_file_generator_cases() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-inline-file-generator");
    run_cptool(
        ["pkg", "init", "stress_plan_inline_file_generator", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_inline_file_generator");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("fixtures").join("input").join("1.in"),
        "3 4\n",
    )
    .unwrap();
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml = yaml.replacen(
        "  tasks:\n  - name: sample\n    score: 100.0\n    type: min\n    bundles: [sample]\n    pass: [brute]\n",
        "  tasks:\n  - name: sample\n    score: 100.0\n    type: min\n    bundles: [sample]\n    pass: [brute]\n  - name: file-corners\n    cases:\n    - generator: :file\n      args: [\"fixtures/input/1.in\"]\n    pass: [brute]\n",
        1,
    );
    std::fs::write(&yaml_path, yaml).unwrap();

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "file-corners",
            "--summary-only",
            "--json",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""task_name":"file-corners:pass:brute""#),
        "{stdout}"
    );
}

#[test]
fn legacy_stress_plan_migrates_to_inline_cases_without_generating_data() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-legacy-stress-plan-inline-migration");
    run_cptool(
        [
            "pkg",
            "init",
            "legacy_stress_plan_inline_migration",
            "--root",
        ],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("legacy_stress_plan_inline_migration");
    configure_python_problem(&problem_dir);
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml.push_str(
        r#"stress:
  plans:
  - name: migrated-proof
    generator: gen
    args: ["{case}", "4"]
    against: [std, brute]
    cases: 2
    expect: pass
"#,
    );
    std::fs::write(&yaml_path, yaml).unwrap();

    let output = run_cptool(
        [
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "migrated-proof",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("task `migrated-proof:pass:brute` case 1 ok"));
    assert!(stdout.contains("task `migrated-proof:pass:brute` case 2 ok"));

    let migrated = std::fs::read_to_string(yaml_path).unwrap();
    assert!(migrated.contains("name: migrated-proof"));
    assert!(migrated.contains("cases:"));
    assert!(!migrated.contains("stress:\n"));
    assert!(!migrated.contains("stress_migrated"));
}

#[test]
fn stress_plan_expect_fail_treats_wrong_answer_as_success() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-expect-fail");
    run_cptool(
        ["pkg", "init", "stress_plan_expect_fail", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("stress_plan_expect_fail");
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
            "test",
            "task",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "bad-is-detected",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("bad-is-detected:fail:bad: expected_fail observed=true case=1"));
    assert!(stdout.contains("reason=wrong_answer: output mismatch between `std` and `bad`"));
    assert!(stdout.contains("failed_cases=3"));
    assert!(stdout.contains("passed_cases=0"));
    assert!(stdout.contains("failure_ratio=1.000"));
    assert!(stdout.contains("cases_run=3"));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(
        problem_dir
            .join(".cptool")
            .join("failures")
            .join("stress-001.txt")
            .exists()
    );

    let json_output = run_cptool(
        [
            "test",
            "task",
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
    let failure = &value["tasks"][0]["expected_failure"];

    assert_eq!(value["tasks"][0]["cases"], 3);
    assert_eq!(value["tasks"][0]["unique_input_hashes"], 1);
    assert_eq!(failure["failed_cases"], 3);
    assert_eq!(failure["passed_cases"], 0);
    assert_eq!(failure["failure_ratio"], 1.0);
    assert!(failure["input_sha256"].as_str().unwrap().len() == 64);
    assert!(
        problem_dir
            .join(failure["input_path"].as_str().unwrap())
            .exists()
    );
    assert!(
        problem_dir
            .join(failure["report_path"].as_str().unwrap())
            .exists()
    );
    assert_eq!(failure["outputs"].as_array().unwrap().len(), 2);
}
