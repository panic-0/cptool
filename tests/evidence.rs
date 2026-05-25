mod common;
use common::*;
use serde_json::Value;

#[test]
fn evidence_json_aggregates_check_gen_and_task() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-json");
    run_cptool(
        ["pkg", "init", "evidence_json_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("evidence_json_problem");
    configure_python_problem(&problem_dir);
    append_legacy_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "report",
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(value["cptool_version"].as_str().unwrap().contains("commit"));
    assert_eq!(value["check"]["status"], "ok");
    assert_eq!(value["check"]["report"]["errors"], 0);
    assert_eq!(value["gen"]["status"], "ok");
    assert_eq!(value["gen"]["report"]["cases"], 1);
    assert_eq!(value["task"]["status"], "ok");
    assert!(
        value["task"]["report"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["task_name"] == "tiny:pass:brute" && task["cases"] == 2)
    );
}

#[test]
fn evidence_markdown_renders_quality_report_section() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-markdown");
    run_cptool(
        ["pkg", "init", "evidence_markdown_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("evidence_markdown_problem");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("bad.py"),
        r#"import sys

a, b = map(int, sys.stdin.read().split())
sys.stdout.buffer.write(f"{a + b + 1}\n".encode("ascii"))
"#,
    )
    .unwrap();
    append_legacy_mixed_stress_plans(&problem_dir);

    let output = run_cptool(
        [
            "report",
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--markdown",
        ],
        None,
    );
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(text.contains("## Tool Evidence"));
    assert!(text.contains("### Check"));
    assert!(text.contains("- status: `pass`"));
    assert!(text.contains("### Generation"));
    assert!(text.contains("- validator_configured: false"));
    assert!(text.contains("### Positive Task Checks"));
    assert!(text.contains("`tiny-pass:pass:brute`: cases=2 unique_input_hashes=1"));
    assert!(text.contains("### Negative Task Checks"));
    assert!(text.contains("`bad-is-detected:fail:bad`: cases=2 unique_input_hashes=1"));
    assert!(text.contains("failed_cases=2 passed_cases=0 failure_ratio=1.000"));
}

#[test]
fn evidence_json_out_writes_utf8_sidecar_and_preserves_stdout() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-json-out 路径");
    run_cptool(
        ["pkg", "init", "evidence_json_out", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("evidence_json_out");
    configure_unicode_python_problem(&problem_dir);
    append_legacy_stress_plan(&problem_dir);
    let out_path = problem_dir
        .join("reports")
        .join("nested")
        .join("evidence.json");

    let output = run_cptool(
        [
            "report",
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
            "--out",
            out_path.to_str().unwrap(),
        ],
        None,
    );
    let stdout_text = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout_text.contains("cptool-evidence-json-out 路径"));
    let sidecar = std::fs::read(&out_path).unwrap();

    assert_eq!(sidecar, output.stdout);
    let value: Value = serde_json::from_slice(&sidecar).unwrap();
    assert_eq!(value["check"]["status"], "ok");
    assert_eq!(value["gen"]["status"], "ok");
    assert_eq!(value["task"]["status"], "ok");
}

#[test]
fn evidence_markdown_out_writes_same_quality_section_as_stdout() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-markdown-out");
    run_cptool(
        ["pkg", "init", "evidence_markdown_out", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("evidence_markdown_out");
    configure_python_problem(&problem_dir);
    append_legacy_stress_plan(&problem_dir);
    let out_path = problem_dir.join("report-evidence.md");

    let output = run_cptool(
        [
            "report",
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--markdown",
            "--out",
            out_path.to_str().unwrap(),
        ],
        None,
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let sidecar = std::fs::read_to_string(&out_path).unwrap();

    assert_eq!(sidecar, stdout);
    assert!(sidecar.contains("## Tool Evidence"));
    assert!(sidecar.contains("### Check"));
    assert!(sidecar.contains("### Generation"));
    assert!(sidecar.contains("### Positive Task Checks"));
}

#[test]
fn evidence_text_out_does_not_replace_existing_directory_on_failure() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-out-failure");
    run_cptool(
        ["pkg", "init", "evidence_out_failure", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("evidence_out_failure");
    configure_python_problem(&problem_dir);
    append_legacy_stress_plan(&problem_dir);
    let out_path = problem_dir.join("existing-target");
    std::fs::create_dir_all(&out_path).unwrap();

    let output = run_cptool_allow_failure(
        [
            "report",
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--out",
            out_path.to_str().unwrap(),
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(out_path.is_dir());
    assert!(stderr.contains("failed to move evidence output temp file"));
}

#[test]
fn evidence_json_can_reuse_task_report_without_new_failure_artifacts() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-reuse-task");
    run_cptool(
        ["pkg", "init", "evidence_reuse_task", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("evidence_reuse_task");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("bad.py"),
        r#"import sys

a, b = map(int, sys.stdin.read().split())
sys.stdout.buffer.write(f"{a + b + 1}\n".encode("ascii"))
"#,
    )
    .unwrap();
    append_legacy_expect_fail_stress_plan(&problem_dir);

    let task = run_cptool(
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
    let reused_path = problem_dir.join("task-summary.json");
    std::fs::write(&reused_path, &task.stdout).unwrap();
    let failure_reports_before = count_failure_reports(&problem_dir);

    let evidence = run_cptool(
        [
            "report",
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
            "--reuse-existing-task",
            reused_path.to_str().unwrap(),
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&evidence.stdout).unwrap();

    assert_eq!(value["task"]["status"], "ok");
    let reused_plan = value["task"]["report"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["task_name"] == "bad-is-detected:fail:bad")
        .unwrap();
    assert_eq!(reused_plan["expected_failure"]["failed_cases"], 3);
    assert_eq!(count_failure_reports(&problem_dir), failure_reports_before);
    assert!(
        !problem_dir
            .join(".cptool")
            .join("failures")
            .join("expect-002.txt")
            .exists()
    );
}
#[test]
fn evidence_json_waits_for_generation_lock_and_stays_parseable() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-json-wait-lock");
    run_cptool(
        ["pkg", "init", "evidence_json_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("evidence_json_wait_lock");
    configure_python_problem(&problem_dir);
    append_legacy_stress_plan(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, GENERATION_LOCK_RELEASE_DELAY);

    let output = run_cptool(
        [
            "report",
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
            "--wait-for-generation-lock",
            GENERATION_LOCK_WAIT_TIMEOUT_SECS,
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    handle.join().unwrap();
    assert_eq!(value["check"]["status"], "ok");
    assert_eq!(value["check"]["report"]["status"], "pass");
    assert_eq!(value["gen"]["status"], "ok");
    assert_eq!(value["task"]["status"], "ok");
    assert!(stderr.contains("waiting for data generation lock:"));
    assert!(stderr.contains(GENERATION_LOCK_WAIT_TIMEOUT_LOG));
}
