mod common;
use common::*;
use serde_json::Value;
use std::time::Duration;

#[test]
fn evidence_json_aggregates_check_gen_and_stress_plan() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-json");
    run_cptool(
        ["init", "evidence_json_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("evidence_json_problem");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        ["evidence", "-w", problem_dir.to_str().unwrap(), "--json"],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(value["cptool_version"].as_str().unwrap().contains("commit"));
    assert_eq!(value["check"]["status"], "ok");
    assert_eq!(value["check"]["report"]["errors"], 0);
    assert_eq!(value["gen"]["status"], "ok");
    assert_eq!(value["gen"]["report"]["cases"], 1);
    assert_eq!(value["stress_plan"]["status"], "ok");
    assert_eq!(value["stress_plan"]["report"][0]["plan_name"], "tiny");
    assert_eq!(value["stress_plan"]["report"][0]["cases"], 2);
}

#[test]
fn evidence_markdown_renders_quality_report_section() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-markdown");
    run_cptool(
        ["init", "evidence_markdown_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("evidence_markdown_problem");
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
    assert!(text.contains("### Positive Stress Plans"));
    assert!(text.contains("`tiny-pass`: cases=2 unique_input_hashes=1"));
    assert!(text.contains("### Negative Stress Plans"));
    assert!(text.contains("`bad-is-detected`: cases=2 unique_input_hashes=1"));
    assert!(text.contains("failed_cases=2 passed_cases=0 failure_ratio=1.000"));
}
#[test]
fn evidence_json_can_reuse_stress_plan_report_without_new_failure_artifacts() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-evidence-reuse-stress-plan");
    run_cptool(
        ["init", "evidence_reuse_stress_plan", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("evidence_reuse_stress_plan");
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

    let stress_plan = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--json",
        ],
        None,
    );
    let reused_path = problem_dir.join("stress-plan-summary.json");
    std::fs::write(&reused_path, &stress_plan.stdout).unwrap();
    let failure_reports_before = count_failure_reports(&problem_dir);

    let evidence = run_cptool(
        [
            "evidence",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
            "--reuse-existing-stress-plan",
            reused_path.to_str().unwrap(),
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&evidence.stdout).unwrap();

    assert_eq!(value["stress_plan"]["status"], "ok");
    assert_eq!(
        value["stress_plan"]["report"][0]["plan_name"],
        "bad-is-detected"
    );
    assert_eq!(
        value["stress_plan"]["report"][0]["expected_failure"]["failed_cases"],
        3
    );
    assert_eq!(count_failure_reports(&problem_dir), failure_reports_before);
    assert!(
        !problem_dir
            .join("tests")
            .join("failures")
            .join("stress-bad-is-detected-002.txt")
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
        ["init", "evidence_json_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("evidence_json_wait_lock");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, Duration::from_millis(500));

    let output = run_cptool(
        [
            "evidence",
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
    assert_eq!(value["check"]["status"], "ok");
    assert_eq!(value["check"]["report"]["status"], "pass");
    assert_eq!(value["gen"]["status"], "ok");
    assert_eq!(value["stress_plan"]["status"], "ok");
    assert!(stderr.contains("waiting for data generation lock:"));
    assert!(stderr.contains("timeout=1s"));
}
