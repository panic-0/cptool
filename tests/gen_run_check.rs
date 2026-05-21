mod common;
use common::*;
use serde_json::Value;
use std::time::Duration;

#[test]
fn run_summary_only_and_hide_stdout_do_not_print_full_stdout() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-run-summary");
    run_cptool(["init", "summary_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("summary_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["gen", "-w"], Some(&problem_dir));

    let summary = run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
        ],
        None,
    );
    let summary_stdout = String::from_utf8_lossy(&summary.stdout);

    assert!(summary_stdout.contains("std: ok exit=0"));
    assert!(summary_stdout.contains("stdout_bytes=2"));
    assert!(summary_stdout.contains("stdout_lines=1"));
    assert!(summary_stdout.contains("stdout_sha256="));
    assert!(summary_stdout.contains("stderr_bytes=0"));
    assert!(!summary_stdout.contains("\n7\n"));

    let hidden = run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--hide-stdout",
        ],
        None,
    );
    let hidden_stdout = String::from_utf8_lossy(&hidden.stdout);

    assert!(hidden_stdout.contains("std: ok exit=0"));
    assert!(!hidden_stdout.contains("\n7\n"));
}
#[test]
fn run_json_prints_machine_readable_summary_without_program_stdout() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-run-json");
    run_cptool(["init", "run_json_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("run_json_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["gen", "-w"], Some(&problem_dir));

    let output = run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(value["ok"], true);
    assert_eq!(value["stdout_bytes"], 2);
    assert_eq!(value["stdout_lines"], 1);
    assert_eq!(value["stderr_nonempty"], false);
    assert!(value.get("stdout").is_none());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("7\n"));
}

#[test]
fn run_can_override_time_and_memory_limits() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-run-limit-override");
    run_cptool(["init", "run_limit_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("run_limit_problem");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        r#"import sys
import time

time.sleep(0.2)
a, b = map(int, sys.stdin.read().split())
sys.stdout.buffer.write(f"{a + b}\n".encode("ascii"))
"#,
    )
    .unwrap();

    let low_limit = run_cptool_allow_failure(
        [
            "run",
            "std",
            "-w",
            problem_dir.to_str().unwrap(),
            "--stdin-text",
            "3 4\n",
            "--time-limit-secs",
            "0.05",
            "--summary-only",
        ],
        None,
    );
    let low_stdout = String::from_utf8_lossy(&low_limit.stdout);
    assert!(!low_limit.status.success());
    assert!(low_stdout.contains("std: timeout exit=none"));

    let high_limit = run_cptool(
        [
            "run",
            "std",
            "-w",
            problem_dir.to_str().unwrap(),
            "--stdin-text",
            "3 4\n",
            "--time-limit-secs",
            "2",
            "--memory-limit-mb",
            "256",
        ],
        None,
    );
    let high_stdout = String::from_utf8_lossy(&high_limit.stdout);
    assert!(high_stdout.contains("std: ok exit=0"));
    assert!(high_stdout.contains("\n7\n"));
}

#[test]
fn gen_warns_on_empty_answer_for_non_empty_input_unless_allowed() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-empty-answer");
    run_cptool(["init", "empty_answer", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("empty_answer");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let result = run_cptool(["gen", "-w"], Some(&problem_dir));
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(stderr.contains("warning: empty_answer"));
    assert!(stderr.contains("case=sample[0]"));
    assert!(stderr.contains("solution=std"));
    assert!(stderr.contains("stdout_bytes=0"));
    assert!(stderr.contains("stderr_bytes=0"));

    let summary = run_cptool(
        ["gen", "-w", problem_dir.to_str().unwrap(), "--summary-only"],
        None,
    );
    let summary_stdout = String::from_utf8_lossy(&summary.stdout);
    let summary_stderr = String::from_utf8_lossy(&summary.stderr);

    assert!(summary_stdout.contains("gen: ok cases=1 bundles=sample elapsed="));
    assert!(summary_stdout.contains("in_bytes=4"));
    assert!(summary_stdout.contains("ans_bytes=0"));
    assert!(summary_stdout.contains("warnings=empty_answer:1"));
    assert!(!summary_stdout.contains("generated "));
    assert!(!summary_stderr.contains("warning: empty_answer"));

    let yaml_path = problem_dir.join("problem.yaml");
    let yaml = std::fs::read_to_string(&yaml_path).unwrap();
    std::fs::write(
        &yaml_path,
        yaml.replacen(
            "programs:\n",
            "output:\n  allow_empty: true\nprograms:\n",
            1,
        ),
    )
    .unwrap();

    let allowed = run_cptool(["gen", "-w"], Some(&problem_dir));
    let allowed_stderr = String::from_utf8_lossy(&allowed.stderr);

    assert!(!allowed_stderr.contains("warning: empty_answer"));
}
#[test]
fn gen_summary_only_prints_compact_success_totals() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-summary");
    run_cptool(["init", "gen_summary", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("gen_summary");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        ["gen", "-w", problem_dir.to_str().unwrap(), "--summary-only"],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("gen: ok cases=1 bundles=sample elapsed="));
    assert!(stdout.contains("in_bytes=4"));
    assert!(stdout.contains("ans_bytes=2"));
    assert!(stdout.contains("warnings=0"));
    assert!(!stdout.contains("generated "));
}
#[test]
fn gen_summary_only_json_prints_report() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-json");
    run_cptool(["init", "gen_json", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("gen_json");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(value["cases"], 1);
    assert_eq!(value["bundles"][0], "sample");
    assert_eq!(value["input_bytes"], 4);
    assert_eq!(value["answer_bytes"], 2);
    assert_eq!(value["validator_configured"], false);
    assert_eq!(value["validator_calls"], 0);
    assert_eq!(value["warnings"].as_array().unwrap().len(), 0);
    assert!(
        value["paths"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .ends_with("sample-0.in")
    );
}
#[test]
fn gen_and_export_cover_multiple_bundles_cases_and_tasks() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-diverse-data");
    run_cptool(["init", "diverse_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("diverse_problem");
    configure_diverse_python_problem(&problem_dir);

    let summary = run_cptool(
        ["gen", "-w", problem_dir.to_str().unwrap(), "--summary-only"],
        None,
    );
    let summary_stdout = String::from_utf8_lossy(&summary.stdout);

    assert!(summary_stdout.contains("gen: ok cases=3 bundles=main,sample elapsed="));
    assert!(summary_stdout.contains("in_bytes=9"));
    assert!(summary_stdout.contains("ans_bytes=12"));
    assert!(summary_stdout.contains("warnings=0"));

    let data_dir = problem_dir.join("data");
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.in")).unwrap(),
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.ans")).unwrap(),
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-0.in")).unwrap(),
        "20\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-0.ans")).unwrap(),
        "400\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-1.in")).unwrap(),
        "300\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-1.ans")).unwrap(),
        "90000\n"
    );

    run_cptool(
        [
            "export",
            "-w",
            problem_dir.to_str().unwrap(),
            "--oj",
            "syzoj",
        ],
        None,
    );

    let export_dir = problem_dir.join("export").join("syzoj");
    assert!(export_dir.join("data.yml").exists());
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.in")).unwrap(),
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.ans")).unwrap(),
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("1.in")).unwrap(),
        "20\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("1.ans")).unwrap(),
        "400\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("2.in")).unwrap(),
        "300\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("2.ans")).unwrap(),
        "90000\n"
    );

    let data_yml = std::fs::read_to_string(export_dir.join("data.yml")).unwrap();
    assert!(data_yml.contains("subtasks:"));
    assert!(data_yml.contains("dependencies:"));
}
#[test]
fn gen_warns_when_generator_stdout_is_empty() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-empty-generator");
    run_cptool(["init", "empty_generator", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("empty_generator");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        "import sys\nsys.stderr.write('no input produced')\n",
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let result = run_cptool(["gen", "-w"], Some(&problem_dir));
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(stderr.contains("warning: generator_output_suspicious"));
    assert!(stderr.contains("case=sample[0]"));
    assert!(stderr.contains("generator=gen"));
    assert!(stderr.contains("stdout_bytes=0"));
    assert!(stderr.contains("stderr_bytes="));
}
#[test]
fn gen_clean_removes_only_selected_bundle_and_preserves_on_failure() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-clean");
    run_cptool(["init", "clean_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("clean_problem");
    configure_python_problem(&problem_dir);

    run_cptool(["gen", "-w"], Some(&problem_dir));
    let data_dir = problem_dir.join("data");
    std::fs::write(data_dir.join("sample-99.in"), "stale").unwrap();
    std::fs::write(data_dir.join("sample-99.ans"), "stale").unwrap();
    std::fs::write(data_dir.join("sampleish-0.in"), "keep").unwrap();

    run_cptool(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--bundle",
            "sample",
            "--clean",
        ],
        None,
    );

    assert!(!data_dir.join("sample-99.in").exists());
    assert!(!data_dir.join("sample-99.ans").exists());
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sampleish-0.in")).unwrap(),
        "keep"
    );

    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        "import sys\nsys.exit(1)\n",
    )
    .unwrap();
    let failed = run_cptool_allow_failure(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--case",
            "sample[0]",
        ],
        None,
    );

    assert!(!failed.status.success());
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.in")).unwrap(),
        "3 4\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.ans")).unwrap(),
        "7\n"
    );
}
#[test]
fn gen_waits_for_generation_lock_when_requested() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-wait-lock");
    run_cptool(["init", "gen_wait_lock", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("gen_wait_lock");
    configure_python_problem(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, Duration::from_millis(500));

    let output = run_cptool(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--wait-for-generation-lock",
            "1",
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    handle.join().unwrap();
    assert!(stderr.contains("waiting for data generation lock:"));
    assert!(stderr.contains("timeout=1s"));
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.in")).unwrap(),
        "3 4\n"
    );
}
#[test]
fn run_waits_for_generation_lock_before_implicit_case_generation() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-run-wait-lock");
    run_cptool(["init", "run_wait_lock", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("run_wait_lock");
    configure_python_problem(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, Duration::from_millis(500));

    let output = run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--wait-for-generation-lock",
            "1",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    handle.join().unwrap();
    assert!(stdout.contains("std: ok"));
    assert!(stdout.contains("7\n"));
    assert!(stderr.contains("waiting for data generation lock:"));
}
#[test]
fn gen_json_reports_validator_stats() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-validator-json");
    run_cptool(["init", "gen_validator_json", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("gen_validator_json");
    configure_python_problem(&problem_dir);
    add_validator_program(&problem_dir, "import sys\nsys.stdin.buffer.read()\n");

    let output = run_cptool(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(value["cases"], 1);
    assert_eq!(value["validator_configured"], true);
    assert_eq!(value["validator_calls"], 1);
}
#[test]
fn gen_validator_failure_reports_case_and_generator_args() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-validator-failure");
    run_cptool(
        ["init", "gen_validator_failure", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("gen_validator_failure");
    configure_python_problem(&problem_dir);
    add_validator_program(&problem_dir, "import sys\nsys.exit(3)\n");

    let output = run_cptool_allow_failure(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--case",
            "sample[0]",
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("validator failed for sample[0]"));
    assert!(stderr.contains("generator=gen"));
    assert!(stderr.contains("args=[\"3\", \"4\"]"));
}
#[test]
fn check_command_reports_valid_and_invalid_packages() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-command");
    run_cptool(["init", "check_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("check_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["gen", "-w"], Some(&problem_dir));

    let ok = run_cptool(["check", "-w"], Some(&problem_dir));
    let ok_stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(ok_stdout.contains("status: `PASS`"));

    std::fs::remove_file(problem_dir.join("src").join("std.cpp")).unwrap();
    let failed = run_cptool_allow_failure(["check", "-w"], Some(&problem_dir));
    let failed_stdout = String::from_utf8_lossy(&failed.stdout);

    assert!(!failed.status.success());
    assert!(failed_stdout.contains("status: `FAIL`"));
    assert!(failed_stdout.contains("required_file_missing"));
}
#[test]
fn check_json_reports_status_and_issue_counts() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-json");
    run_cptool(["init", "check_json_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("check_json_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["gen", "-w"], Some(&problem_dir));

    let ok = run_cptool(
        ["check", "-w", problem_dir.to_str().unwrap(), "--json"],
        None,
    );
    let ok_value: Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(ok_value["status"], "pass");
    assert_eq!(ok_value["errors"], 0);
    assert!(ok_value.get("schema_version").is_none());

    std::fs::remove_file(problem_dir.join("src").join("std.cpp")).unwrap();
    let failed = run_cptool_allow_failure(
        ["check", "-w", problem_dir.to_str().unwrap(), "--json"],
        None,
    );
    let failed_value: Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert!(!failed.status.success());
    assert_eq!(failed_value["status"], "fail");
    assert!(failed_value["errors"].as_u64().unwrap() > 0);
    assert!(
        failed_value["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| {
                issue["code"] == "required_file_missing" && issue["severity"] == "error"
            })
    );
}
#[test]
fn check_json_reports_missing_and_stale_generated_data() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-data-audit");
    run_cptool(["init", "check_data_audit", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("check_data_audit");
    configure_python_problem(&problem_dir);

    let missing = run_cptool_allow_failure(
        ["check", "-w", problem_dir.to_str().unwrap(), "--json"],
        None,
    );
    let missing_value: Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert!(!missing.status.success());
    assert!(
        missing_value["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "generated_data_missing" && issue["severity"] == "error")
    );

    run_cptool(["gen", "-w"], Some(&problem_dir));
    let data_dir = problem_dir.join("data");
    std::fs::write(data_dir.join("sample-99.in"), "stale\n").unwrap();
    std::fs::write(data_dir.join("unknown-0.ans"), "stale\n").unwrap();
    std::fs::write(data_dir.join("badname.in"), "stale\n").unwrap();

    let stale = run_cptool(
        ["check", "-w", problem_dir.to_str().unwrap(), "--json"],
        None,
    );
    let stale_value: Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(stale_value["status"], "pass");
    assert_eq!(stale_value["errors"], 0);
    assert!(
        stale_value["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "stale_data_file" && issue["severity"] == "warning")
    );
}
#[test]
fn check_json_marks_generation_lock_as_transient() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-json-lock");
    run_cptool(["init", "check_json_lock", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("check_json_lock");
    configure_python_problem(&problem_dir);
    std::fs::create_dir_all(problem_dir.join("data").join(".cptool-gen.lock")).unwrap();

    let output = run_cptool_allow_failure(
        ["check", "-w", problem_dir.to_str().unwrap(), "--json"],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let lock_issue = value["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|issue| issue["code"] == "data_generation_in_progress")
        .expect("expected generation lock issue");

    assert!(!output.status.success());
    assert_eq!(lock_issue["kind"], "lock");
    assert_eq!(lock_issue["transient"], true);
    assert_eq!(lock_issue["retry_after"], "wait_for_generation_then_retry");
}
#[test]
fn check_json_waits_for_generation_lock_and_stays_parseable() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-json-wait-lock");
    run_cptool(
        ["init", "check_json_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("check_json_wait_lock");
    configure_python_problem(&problem_dir);
    run_cptool(["gen", "-w"], Some(&problem_dir));
    let handle = release_generation_lock_after(&problem_dir, Duration::from_millis(500));

    let output = run_cptool(
        [
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
            "--wait-for-generation-lock",
            "1",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    handle.join().unwrap();
    assert_eq!(value["status"], "pass");
    assert!(
        !value["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| { issue["code"] == "data_generation_in_progress" })
    );
    assert!(stderr.contains("waiting for data generation lock:"));
}
