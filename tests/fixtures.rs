mod common;
use common::*;
use serde_json::Value;

#[test]
fn fixture_add_input_and_check_requires_usage() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-fixture-input");
    run_cptool(
        ["pkg", "init", "fixture_input", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("fixture_input");
    configure_python_problem(&problem_dir);
    std::fs::write(problem_dir.join("source.in"), "2 5\n").unwrap();

    run_cptool(
        [
            "fixture",
            "add",
            "input",
            "small",
            "-w",
            problem_dir.to_str().unwrap(),
            "--from",
            "source.in",
        ],
        None,
    );
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("fixtures").join("input").join("small.in"))
            .unwrap(),
        "2 5\n"
    );

    let unused = run_cptool_allow_failure(
        [
            "fixture",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    assert_eq!(unused.status.code(), Some(2));
    let unused_value: Value = serde_json::from_slice(&unused.stdout).unwrap();
    assert_eq!(unused_value["ok"], false);
    assert_eq!(unused_value["errors"][0]["code"], "unused_input_fixture");

    std::fs::write(
        problem_dir.join("problem.yaml"),
        r#"name: fixture_input
programs:
  std:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
validator_omitted_reason: "fixture input smoke test"
test:
  bundles:
    corner:
      cases:
      - generator: :file
        args: [fixtures/input/small.in]
  tasks:
  - name: corner
    score: 100.0
    type: min
    bundles: [corner]
"#,
    )
    .unwrap();
    let used = run_cptool(
        [
            "fixture",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let used_value: Value = serde_json::from_slice(&used.stdout).unwrap();
    assert_eq!(used_value["ok"], true);
    assert_eq!(used_value["list"]["inputs"][0]["used"], true);
}

#[test]
fn fixture_add_reports_actual_replacement() {
    let temp = TempWorkspace::new("cptool-fixture-replace-report");
    run_cptool(
        ["pkg", "init", "fixture_replace", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("fixture_replace");

    let create_with_replace = run_cptool(
        [
            "fixture",
            "add",
            "input",
            "small",
            "-w",
            problem_dir.to_str().unwrap(),
            "--replace",
        ],
        None,
    );
    assert!(
        String::from_utf8_lossy(&create_with_replace.stdout).contains("created"),
        "{}",
        String::from_utf8_lossy(&create_with_replace.stdout)
    );

    let replace_existing = run_cptool(
        [
            "fixture",
            "add",
            "input",
            "small",
            "-w",
            problem_dir.to_str().unwrap(),
            "--replace",
        ],
        None,
    );
    assert!(
        String::from_utf8_lossy(&replace_existing.stdout).contains("wrote"),
        "{}",
        String::from_utf8_lossy(&replace_existing.stdout)
    );
}

#[test]
fn test_validator_runs_all_validator_fixtures() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-fixture-validator");
    run_cptool(
        ["pkg", "init", "fixture_validator", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("fixture_validator");
    configure_python_problem(&problem_dir);
    add_validator_program(
        &problem_dir,
        r#"import sys
data = sys.stdin.read().strip()
if data != "ok":
    raise SystemExit(3)
"#,
    );
    std::fs::write(problem_dir.join("good.in"), "ok\n").unwrap();
    std::fs::write(problem_dir.join("bad.in"), "bad\n").unwrap();

    run_cptool(
        [
            "fixture",
            "add",
            "validator",
            "pass",
            "good",
            "-w",
            problem_dir.to_str().unwrap(),
            "--from",
            "good.in",
        ],
        None,
    );
    run_cptool(
        [
            "fixture",
            "add",
            "validator",
            "fail",
            "bad",
            "-w",
            problem_dir.to_str().unwrap(),
            "--from",
            "bad.in",
        ],
        None,
    );

    let output = run_cptool(
        [
            "test",
            "validator",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["role"], "validator");
    assert_eq!(value["ok"], true);
    assert_eq!(value["total"], 2);
    assert_eq!(value["fixtures"].as_array().unwrap().len(), 2);
}

#[test]
fn test_checker_runs_all_checker_fixtures() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-fixture-checker");
    run_cptool(
        ["pkg", "init", "fixture_checker", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("fixture_checker");
    configure_checker_python_problem(&problem_dir);
    std::fs::write(problem_dir.join("input.in"), "7\n").unwrap();
    std::fs::write(problem_dir.join("answer.ans"), "7\n").unwrap();
    std::fs::write(problem_dir.join("good.out"), "007\n").unwrap();
    std::fs::write(problem_dir.join("bad.out"), "8\n").unwrap();

    run_cptool(
        [
            "fixture",
            "add",
            "checker",
            "pass",
            "good",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "input.in",
            "--output",
            "good.out",
            "--answer",
            "answer.ans",
        ],
        None,
    );
    assert_eq!(
        std::fs::read_to_string(
            problem_dir
                .join("fixtures")
                .join("checker")
                .join("pass")
                .join("good.in")
        )
        .unwrap(),
        "7\n"
    );
    assert_eq!(
        std::fs::read_to_string(
            problem_dir
                .join("fixtures")
                .join("checker")
                .join("pass")
                .join("good.out")
        )
        .unwrap(),
        "007\n"
    );
    assert_eq!(
        std::fs::read_to_string(
            problem_dir
                .join("fixtures")
                .join("checker")
                .join("pass")
                .join("good.ans")
        )
        .unwrap(),
        "7\n"
    );
    run_cptool(
        [
            "fixture",
            "add",
            "checker",
            "fail",
            "bad",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "input.in",
            "--output",
            "bad.out",
            "--answer",
            "answer.ans",
        ],
        None,
    );

    let output = run_cptool(
        [
            "test",
            "checker",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["role"], "checker");
    assert_eq!(value["ok"], true);
    assert_eq!(value["total"], 2);
}

#[test]
fn checker_fixture_requires_sources_and_check_rejects_empty_files() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-fixture-checker-empty");
    run_cptool(
        ["pkg", "init", "fixture_checker_empty", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("fixture_checker_empty");
    configure_checker_python_problem(&problem_dir);

    let missing_sources = run_cptool_allow_failure(
        [
            "fixture",
            "add",
            "checker",
            "pass",
            "empty",
            "-w",
            problem_dir.to_str().unwrap(),
        ],
        None,
    );
    assert!(!missing_sources.status.success());
    let stderr = String::from_utf8_lossy(&missing_sources.stderr);
    assert!(stderr.contains("requires --input"), "{stderr}");

    let fixture_stem = problem_dir
        .join("fixtures")
        .join("checker")
        .join("pass")
        .join("empty");
    std::fs::create_dir_all(fixture_stem.parent().unwrap()).unwrap();
    std::fs::write(fixture_stem.with_extension("in"), "").unwrap();
    std::fs::write(fixture_stem.with_extension("out"), "7\n").unwrap();
    std::fs::write(fixture_stem.with_extension("ans"), "7\n").unwrap();

    let check = run_cptool_allow_failure(
        [
            "fixture",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    assert_eq!(check.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(value["errors"][0]["code"], "empty_checker_fixture_file");
}

#[test]
fn test_checker_reports_missing_explicit_paths() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-fixture-checker-missing-explicit");
    run_cptool(
        ["pkg", "init", "fixture_checker_missing_explicit", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("fixture_checker_missing_explicit");
    configure_checker_python_problem(&problem_dir);

    let output = run_cptool_allow_failure(
        [
            "test",
            "checker",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "input.in",
        ],
        None,
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing --output and --answer"), "{stderr}");
}
