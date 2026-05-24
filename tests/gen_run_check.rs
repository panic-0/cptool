mod common;
use common::*;
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn run_summary_only_and_hide_stdout_do_not_print_full_stdout() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-run-summary");
    run_cptool(
        ["pkg", "init", "summary_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("summary_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

    let summary = run_cptool(
        [
            "case",
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
            "case",
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
    run_cptool(
        ["pkg", "init", "run_json_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("run_json_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

    let output = run_cptool(
        [
            "case",
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
fn judge_validator_accepts_input_file_and_expect_fail() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-judge-validator");
    run_cptool(
        ["pkg", "init", "judge_validator", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("judge_validator");
    configure_python_problem(&problem_dir);
    add_validator_program(
        &problem_dir,
        r#"import sys
data = sys.stdin.read().strip()
if data != "ok":
    raise SystemExit(3)
"#,
    );
    std::fs::write(problem_dir.join("valid.in"), "ok\n").unwrap();
    std::fs::write(problem_dir.join("invalid.in"), "bad\n").unwrap();

    let pass = run_cptool(
        [
            "test",
            "validator",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "valid.in",
            "--json",
        ],
        None,
    );
    let pass_value: Value = serde_json::from_slice(&pass.stdout).unwrap();
    assert_eq!(pass_value["role"], "validator");
    assert_eq!(pass_value["ok"], true);
    assert_eq!(pass_value["observed"], "pass");

    let fail = run_cptool(
        [
            "test",
            "validator",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "invalid.in",
            "--expect",
            "fail",
            "--json",
        ],
        None,
    );
    let fail_value: Value = serde_json::from_slice(&fail.stdout).unwrap();
    assert_eq!(fail_value["ok"], true);
    assert_eq!(fail_value["expect"], "fail");
    assert_eq!(fail_value["observed"], "fail");

    let unexpected = run_cptool_allow_failure(
        [
            "test",
            "validator",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "invalid.in",
        ],
        None,
    );
    assert_eq!(unexpected.status.code(), Some(2));
}

#[test]
fn judge_validator_normalizes_input_line_endings_by_default() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-judge-validator-line-endings");
    run_cptool(
        ["pkg", "init", "judge_validator_line_endings", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("judge_validator_line_endings");
    configure_python_problem(&problem_dir);
    let expected = if cfg!(windows) {
        "b'ok\\r\\nnext\\r\\n'"
    } else {
        "b'ok\\nnext\\n'"
    };
    add_validator_program(
        &problem_dir,
        &format!(
            r#"import sys
data = sys.stdin.buffer.read()
if data != {expected}:
    raise SystemExit(3)
"#
        ),
    );
    let fixture = problem_dir.join("line_endings.in");
    if cfg!(windows) {
        std::fs::write(&fixture, b"ok\nnext\n").unwrap();
    } else {
        std::fs::write(&fixture, b"ok\r\nnext\r\n").unwrap();
    }

    let pass = run_cptool(
        [
            "test",
            "validator",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "line_endings.in",
            "--json",
        ],
        None,
    );
    let pass_value: Value = serde_json::from_slice(&pass.stdout).unwrap();
    assert_eq!(pass_value["ok"], true);
    assert_eq!(
        pass_value["warnings"][0]["code"],
        "input_line_endings_normalized"
    );
    let native = if cfg!(windows) {
        b"ok\r\nnext\r\n".as_slice()
    } else {
        b"ok\nnext\n".as_slice()
    };
    assert_eq!(std::fs::read(&fixture).unwrap(), native);

    let disabled_fixture = problem_dir.join("line_endings_disabled.in");
    if cfg!(windows) {
        std::fs::write(&disabled_fixture, b"ok\nnext\n").unwrap();
    } else {
        std::fs::write(&disabled_fixture, b"ok\r\nnext\r\n").unwrap();
    }
    let disabled = run_cptool_allow_failure(
        [
            "test",
            "validator",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "line_endings_disabled.in",
            "--no-fix-line-endings",
            "--json",
        ],
        None,
    );
    assert_eq!(disabled.status.code(), Some(2));
    let disabled_value: Value = serde_json::from_slice(&disabled.stdout).unwrap();
    assert_eq!(disabled_value["ok"], false);
    assert_eq!(disabled_value["warnings"].as_array().unwrap().len(), 0);
    let non_native = if cfg!(windows) {
        b"ok\nnext\n".as_slice()
    } else {
        b"ok\r\nnext\r\n".as_slice()
    };
    assert_eq!(std::fs::read(&disabled_fixture).unwrap(), non_native);
}

#[test]
fn gen_file_generator_copies_fixture_and_writes_answer() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-file-generator");
    run_cptool(
        ["pkg", "init", "file_generator", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("file_generator");
    configure_python_problem(&problem_dir);
    let yaml_path = problem_dir.join("problem.yaml");
    std::fs::write(
        &yaml_path,
        r#"name: file_generator
programs:
  std:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
validator_omitted_reason: "file generator smoke test"
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
    std::fs::write(
        problem_dir.join("fixtures").join("input").join("small.in"),
        "2 5\n",
    )
    .unwrap();

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

    let expected_input = if cfg!(windows) { "2 5\r\n" } else { "2 5\n" };
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("corner-0.in")).unwrap(),
        expected_input
    );
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("corner-0.ans")).unwrap(),
        "7\n"
    );
}

#[test]
fn gen_file_generator_can_be_problem_default_and_normalizes_copied_input() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-file-generator-default");
    run_cptool(
        ["pkg", "init", "file_generator_default", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("file_generator_default");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("problem.yaml"),
        r#"name: file_generator_default
programs:
  std:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  val:
    info: !cpp
      path: ./src/val.cpp
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
validator: val
generator: :file
test:
  bundles:
    sample:
      cases:
      - [fixtures/input/strict.in]
  tasks:
  - name: sample
    score: 100.0
    type: min
    bundles: [sample]
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("val.cpp"),
        r#"#include "testlib.h"

int main(int argc, char *argv[]) {
    registerValidation(argc, argv);
    std::string line = inf.readLine();
    ensuref(line == "2 5", "unexpected line");
    inf.readEof();
    return 0;
}
"#,
    )
    .unwrap();
    let fixture = problem_dir.join("fixtures").join("input").join("strict.in");
    std::fs::write(&fixture, b"2 5").unwrap();

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

    let native = if cfg!(windows) {
        b"2 5\r\n".as_slice()
    } else {
        b"2 5\n".as_slice()
    };
    assert_eq!(
        std::fs::read(problem_dir.join("data").join("sample-0.in")).unwrap(),
        native
    );
    assert_eq!(std::fs::read(&fixture).unwrap(), b"2 5");
}

#[test]
fn gen_file_generator_reports_bad_arguments_and_missing_files() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-file-generator-errors");
    run_cptool(
        ["pkg", "init", "file_generator_errors", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("file_generator_errors");
    configure_python_problem(&problem_dir);

    for (case_line, expected) in [
        (
            r#"      - generator: :file
        args: []
"#,
            "expects exactly one input path argument, got 0",
        ),
        (
            r#"      - generator: :file
        args: [missing.in, extra.in]
"#,
            "expects exactly one input path argument, got 2",
        ),
        (
            r#"      - generator: :file
        args: [fixtures/input/missing.in]
"#,
            "failed to read `:file` input",
        ),
        (
            r#"      - generator: :file
        args: [data/manual.in]
"#,
            "must read handwritten input from fixtures/input",
        ),
    ] {
        std::fs::write(
            problem_dir.join("problem.yaml"),
            format!(
                r#"name: file_generator_errors
programs:
  std:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
validator_omitted_reason: "file generator error smoke test"
test:
  bundles:
    sample:
      cases:
{case_line}  tasks:
  - name: sample
    score: 100.0
    type: min
    bundles: [sample]
"#
            ),
        )
        .unwrap();

        let output = run_cptool_allow_failure(["case", "gen", "-w"], Some(&problem_dir));
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(stderr.contains(expected), "{stderr}");
    }
}

#[test]
fn judge_checker_runs_with_file_paths_and_no_stdin_text() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-judge-checker");
    run_cptool(
        ["pkg", "init", "judge_checker", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("judge_checker");
    configure_checker_python_problem(&problem_dir);
    std::fs::write(problem_dir.join("input.in"), "7\n").unwrap();
    std::fs::write(problem_dir.join("answer.ans"), "7\n").unwrap();
    std::fs::write(problem_dir.join("good.out"), "007\n").unwrap();
    std::fs::write(problem_dir.join("bad.out"), "8\n").unwrap();

    let pass = run_cptool(
        [
            "test",
            "checker",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "input.in",
            "--output",
            "good.out",
            "--answer",
            "answer.ans",
            "--json",
        ],
        None,
    );
    let pass_value: Value = serde_json::from_slice(&pass.stdout).unwrap();
    assert_eq!(pass_value["role"], "checker");
    assert_eq!(pass_value["ok"], true);
    assert_eq!(pass_value["observed"], "pass");

    let fail = run_cptool(
        [
            "test",
            "checker",
            "-w",
            problem_dir.to_str().unwrap(),
            "--input",
            "input.in",
            "--output",
            "bad.out",
            "--answer",
            "answer.ans",
            "--expect",
            "fail",
            "--json",
        ],
        None,
    );
    let fail_value: Value = serde_json::from_slice(&fail.stdout).unwrap();
    assert_eq!(fail_value["ok"], true);
    assert_eq!(fail_value["observed"], "fail");
    assert!(
        fail_value["report"]
            .as_str()
            .unwrap()
            .contains("expected 7")
    );

    let help = run_cptool(["test", "validator", "--help"], None);
    assert!(!String::from_utf8_lossy(&help.stdout).contains("--stdin-text"));
}

#[test]
fn run_can_override_time_and_memory_limits() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-run-limit-override");
    run_cptool(
        ["pkg", "init", "run_limit_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("run_limit_problem");
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
            "case",
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
            "case",
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
    run_cptool(["pkg", "init", "empty_answer", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("empty_answer");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let result = run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(stderr.contains("warning: empty_answer"));
    assert!(stderr.contains("case=sample[0]"));
    assert!(stderr.contains("solution=std"));
    assert!(stderr.contains("stdout_bytes=0"));
    assert!(stderr.contains("stderr_bytes=0"));

    let summary = run_cptool(
        [
            "case",
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
        ],
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

    let allowed = run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let allowed_stderr = String::from_utf8_lossy(&allowed.stderr);

    assert!(!allowed_stderr.contains("warning: empty_answer"));
}
#[test]
fn gen_summary_only_prints_compact_success_totals() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-summary");
    run_cptool(["pkg", "init", "gen_summary", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("gen_summary");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "case",
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
        ],
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
fn gen_default_output_prints_paths_relative_to_work_dir() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-short-paths");
    run_cptool(
        ["pkg", "init", "gen_short_paths", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("gen_short_paths");
    configure_python_problem(&problem_dir);

    let output = run_cptool(["case", "gen", "-w", problem_dir.to_str().unwrap()], None);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let long_problem_dir = problem_dir.display().to_string();

    assert!(stdout.contains("generated data/sample-0.in"));
    assert!(stdout.contains("generated data/sample-0.ans"));
    assert!(!stdout.contains(&long_problem_dir));

    let summary = run_cptool(
        [
            "case",
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
        ],
        None,
    );
    let summary_stdout = String::from_utf8_lossy(&summary.stdout);
    assert!(summary_stdout.contains("gen: ok cases=1 bundles=sample elapsed="));
    assert!(!summary_stdout.contains("generated "));
}

#[test]
fn gen_summary_only_json_prints_report() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-json");
    run_cptool(["pkg", "init", "gen_json", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("gen_json");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        [
            "case",
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
    assert!(value["paths"].as_array().unwrap()[0].as_str().unwrap() == "data/sample-0.in");
    assert!(value["paths"].as_array().unwrap()[1].as_str().unwrap() == "data/sample-0.ans");
}
#[test]
fn gen_and_export_cover_multiple_bundles_cases_and_tasks() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-diverse-data");
    run_cptool(
        ["pkg", "init", "diverse_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("diverse_problem");
    configure_diverse_python_problem(&problem_dir);

    let summary = run_cptool(
        [
            "case",
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
        ],
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
            "pkg",
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
    run_cptool(
        ["pkg", "init", "empty_generator", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("empty_generator");
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

    let result = run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(stderr.contains("warning: generator_output_suspicious"));
    assert!(stderr.contains("case=sample[0]"));
    assert!(stderr.contains("generator=gen"));
    assert!(stderr.contains("stdout_bytes=0"));
    assert!(stderr.contains("stderr_bytes="));
}
#[test]
fn gen_rebuilds_data_dir_and_preserves_on_failure() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-rebuild-data");
    run_cptool(
        ["pkg", "init", "rebuild_data_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("rebuild_data_problem");
    configure_python_problem(&problem_dir);

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let data_dir = problem_dir.join("data");
    std::fs::write(data_dir.join("sample-99.in"), "stale").unwrap();
    std::fs::write(data_dir.join("sample-99.ans"), "stale").unwrap();
    std::fs::write(data_dir.join("manual.in"), "handwritten").unwrap();
    std::fs::create_dir_all(data_dir.join("manual")).unwrap();
    std::fs::write(data_dir.join("manual").join("case.in"), "handwritten").unwrap();

    run_cptool(
        [
            "case",
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--bundle",
            "sample",
        ],
        None,
    );

    assert!(!data_dir.join("sample-99.in").exists());
    assert!(!data_dir.join("sample-99.ans").exists());
    assert!(!data_dir.join("manual.in").exists());
    assert!(!data_dir.join("manual").exists());

    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        "import sys\nsys.exit(1)\n",
    )
    .unwrap();
    let failed = run_cptool_allow_failure(
        [
            "case",
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
fn clean_command_removes_data_files_and_cache() {
    let temp = TempWorkspace::new("cptool-clean-command");
    run_cptool(
        ["pkg", "init", "clean_command_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("clean_command_problem");
    let data_dir = problem_dir.join("data");
    let cache_dir = problem_dir.join(".cptool").join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(data_dir.join("sample-0.in"), "input").unwrap();
    std::fs::write(data_dir.join("sample-0.ans"), "answer").unwrap();
    std::fs::write(data_dir.join("notes.txt"), "keep").unwrap();
    std::fs::write(cache_dir.join("artifact"), "cached").unwrap();

    let output = run_cptool(
        [
            "pkg",
            "clean",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(value["data_files_removed"], 2);
    assert_eq!(value["cache_removed"], true);
    assert!(!data_dir.join("sample-0.in").exists());
    assert!(!data_dir.join("sample-0.ans").exists());
    assert_eq!(
        std::fs::read_to_string(data_dir.join("notes.txt")).unwrap(),
        "keep"
    );
    assert!(!cache_dir.exists());
}

#[test]
fn clean_command_can_target_only_data_or_cache() {
    let temp = TempWorkspace::new("cptool-clean-targets");
    run_cptool(
        ["pkg", "init", "clean_targets_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("clean_targets_problem");
    let data_dir = problem_dir.join("data");
    let cache_dir = problem_dir.join(".cptool").join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(data_dir.join("sample-0.in"), "input").unwrap();
    std::fs::write(cache_dir.join("artifact"), "cached").unwrap();

    let data_only = run_cptool(
        [
            "pkg",
            "clean",
            "-w",
            problem_dir.to_str().unwrap(),
            "--data",
        ],
        None,
    );
    assert!(String::from_utf8_lossy(&data_only.stdout).contains("data_files=1"));
    assert!(!data_dir.join("sample-0.in").exists());
    assert!(cache_dir.exists());

    std::fs::write(data_dir.join("sample-0.ans"), "answer").unwrap();
    let cache_only = run_cptool(
        [
            "pkg",
            "clean",
            "-w",
            problem_dir.to_str().unwrap(),
            "--cache",
        ],
        None,
    );
    assert!(String::from_utf8_lossy(&cache_only.stdout).contains("cache_removed=true"));
    assert!(data_dir.join("sample-0.ans").exists());
    assert!(!cache_dir.exists());
}

#[test]
fn clean_command_refuses_during_data_generation() {
    let temp = TempWorkspace::new("cptool-clean-lock");
    run_cptool(
        ["pkg", "init", "clean_lock_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("clean_lock_problem");
    let data_dir = problem_dir.join("data");
    std::fs::write(data_dir.join("sample-0.in"), "input").unwrap();
    std::fs::write(data_dir.join("sample-0.ans"), "answer").unwrap();
    std::fs::create_dir_all(data_dir.join(".cptool-gen.lock")).unwrap();

    let output = run_cptool_allow_failure(
        [
            "pkg",
            "clean",
            "-w",
            problem_dir.to_str().unwrap(),
            "--data",
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("data generation is in progress"));
    assert!(data_dir.join("sample-0.in").exists());
    assert!(data_dir.join("sample-0.ans").exists());
}

#[test]
fn clean_command_refuses_when_staging_dir_exists() {
    let temp = TempWorkspace::new("cptool-clean-staging");
    run_cptool(
        ["pkg", "init", "clean_staging_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("clean_staging_problem");
    let data_dir = problem_dir.join("data");
    std::fs::write(data_dir.join("sample-0.in"), "input").unwrap();
    std::fs::create_dir_all(data_dir.join(".cptool-gen-leftover")).unwrap();

    let output = run_cptool_allow_failure(
        [
            "pkg",
            "clean",
            "-w",
            problem_dir.to_str().unwrap(),
            "--data",
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("data generation is in progress"));
    assert!(data_dir.join("sample-0.in").exists());
}

#[test]
fn gen_waits_for_generation_lock_when_requested() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-wait-lock");
    run_cptool(
        ["pkg", "init", "gen_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("gen_wait_lock");
    configure_python_problem(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, GENERATION_LOCK_RELEASE_DELAY);

    let output = run_cptool(
        [
            "case",
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--wait-for-generation-lock",
            GENERATION_LOCK_WAIT_TIMEOUT_SECS,
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    handle.join().unwrap();
    assert!(stderr.contains("waiting for data generation lock:"));
    assert!(stderr.contains(GENERATION_LOCK_WAIT_TIMEOUT_LOG));
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
    run_cptool(
        ["pkg", "init", "run_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("run_wait_lock");
    configure_python_problem(&problem_dir);
    let handle = release_generation_lock_after(&problem_dir, GENERATION_LOCK_RELEASE_DELAY);

    let output = run_cptool(
        [
            "case",
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--wait-for-generation-lock",
            GENERATION_LOCK_WAIT_TIMEOUT_SECS,
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
    run_cptool(
        ["pkg", "init", "gen_validator_json", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("gen_validator_json");
    configure_python_problem(&problem_dir);
    add_validator_program(&problem_dir, "import sys\nsys.stdin.buffer.read()\n");

    let output = run_cptool(
        [
            "case",
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
        ["pkg", "init", "gen_validator_failure", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("gen_validator_failure");
    configure_python_problem(&problem_dir);
    add_validator_program(&problem_dir, "import sys\nsys.exit(3)\n");

    let output = run_cptool_allow_failure(
        [
            "case",
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
    run_cptool(
        ["pkg", "init", "check_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("check_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

    let ok = run_cptool(["pkg", "check", "-w"], Some(&problem_dir));
    let ok_stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(ok_stdout.contains("status: `PASS`"));

    std::fs::remove_file(problem_dir.join("src").join("std.cpp")).unwrap();
    let failed = run_cptool_allow_failure(["pkg", "check", "-w"], Some(&problem_dir));
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
    run_cptool(
        ["pkg", "init", "check_json_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("check_json_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

    let ok = run_cptool(
        [
            "pkg",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let ok_value: Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(
        json_object_keys(&ok_value),
        ["errors", "issues", "status", "warnings", "work_dir"]
            .into_iter()
            .collect()
    );
    assert_eq!(ok_value["status"], "pass");
    assert_eq!(ok_value["errors"], 0);
    assert_eq!(ok_value["warnings"], issue_count(&ok_value, "warning"));
    assert!(
        ok_value["work_dir"]
            .as_str()
            .unwrap()
            .ends_with("check_json_problem")
    );
    assert!(ok_value["issues"].as_array().unwrap().iter().any(|issue| {
        issue["code"] == "validator_missing"
            && issue["severity"] == "warning"
            && issue["message"].is_string()
            && issue["path"].as_str().unwrap().ends_with("problem.yaml")
    }));
    assert!(ok_value.get("schema_version").is_none());

    std::fs::remove_file(problem_dir.join("src").join("std.cpp")).unwrap();
    let failed = run_cptool_allow_failure(
        [
            "pkg",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let failed_value: Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert!(!failed.status.success());
    assert_eq!(failed.status.code(), Some(2));
    assert_eq!(
        json_object_keys(&failed_value),
        ["errors", "issues", "status", "warnings", "work_dir"]
            .into_iter()
            .collect()
    );
    assert_eq!(failed_value["status"], "fail");
    assert_eq!(failed_value["errors"], issue_count(&failed_value, "error"));
    assert_eq!(
        failed_value["warnings"],
        issue_count(&failed_value, "warning")
    );
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
fn check_json_keeps_package_audit_warning_codes_stable() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-json-package-contract");
    run_cptool(
        ["pkg", "init", "package_contract", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("package_contract");
    configure_python_problem(&problem_dir);
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml.push_str(
        "stress:\n  plans:\n  - name: wrong-proof\n    generator: gen\n    against: [std, brute]\n    cases: 1\n    expect: fail\n",
    );
    std::fs::write(&yaml_path, yaml).unwrap();
    run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    std::fs::write(problem_dir.join("statement.md"), "# Statement\nTODO\n").unwrap();
    std::fs::create_dir_all(problem_dir.join("package_contract")).unwrap();
    std::fs::write(
        problem_dir.join("package_contract").join("problem.yaml"),
        "name: nested\n",
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("quality_report.md"),
        "正向覆盖 wrong-proof\nmissing .cptool/failures/nope.txt\nrate limit\n",
    )
    .unwrap();

    let output = run_cptool(
        [
            "pkg",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(value["status"], "pass");
    for code in [
        "placeholder_text",
        "double_nested_problem_dir",
        "service_side_noise",
        "missing_failure_reference",
        "negative_plan_counted_as_positive",
    ] {
        let issue = find_issue(&value, code);
        assert_eq!(issue["severity"], "warning");
        assert!(issue["message"].is_string());
        assert!(issue["path"].is_string());
    }
}

#[test]
fn check_json_reports_missing_and_stale_generated_data() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-data-audit");
    run_cptool(
        ["pkg", "init", "check_data_audit", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("check_data_audit");
    let display_problem_dir = problem_dir.display().to_string().replace('\\', "/");
    configure_python_problem(&problem_dir);

    let missing = run_cptool_allow_failure(
        [
            "pkg",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let missing_value: Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert!(!missing.status.success());
    let missing_issue = find_issue(&missing_value, "generated_data_missing");
    assert_eq!(missing_issue["severity"], "error");
    assert_eq!(missing_issue["kind"], "not_generated");
    assert!(
        missing_issue["message"]
            .as_str()
            .unwrap()
            .contains("no generated .in/.ans files are present")
    );
    assert_eq!(
        missing_issue["next_action"],
        format!("cptool case gen -w {display_problem_dir}")
    );

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let data_dir = problem_dir.join("data");
    std::fs::write(data_dir.join("sample-99.in"), "stale\n").unwrap();
    std::fs::write(data_dir.join("unknown-0.ans"), "stale\n").unwrap();
    std::fs::write(data_dir.join("badname.in"), "stale\n").unwrap();

    let stale = run_cptool(
        [
            "pkg",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
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
    let stale_issue = find_issue(&stale_value, "stale_data_file");
    assert_eq!(stale_issue["kind"], "stale");
    assert_eq!(
        stale_issue["next_action"],
        format!("cptool case gen -w {display_problem_dir}")
    );
}

#[test]
fn check_text_reports_generated_data_next_action() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-data-next-action");
    run_cptool(
        ["pkg", "init", "check_data_next_action", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("check_data_next_action");
    let display_problem_dir = problem_dir.display().to_string().replace('\\', "/");
    configure_python_problem(&problem_dir);

    let output = run_cptool_allow_failure(["pkg", "check", "-w"], Some(&problem_dir));
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!output.status.success());
    assert!(stdout.contains("generated_data_missing"));
    assert!(stdout.contains(&format!(
        "next action: `cptool case gen -w {display_problem_dir}`"
    )));
}

fn json_object_keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

fn find_issue<'a>(value: &'a Value, code: &str) -> &'a Value {
    value["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|issue| issue["code"] == code)
        .unwrap_or_else(|| panic!("missing issue code `{code}`"))
}

fn issue_count(value: &Value, severity: &str) -> usize {
    value["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|issue| issue["severity"] == severity)
        .count()
}
#[test]
fn check_json_marks_generation_lock_as_transient() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-json-lock");
    run_cptool(
        ["pkg", "init", "check_json_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("check_json_lock");
    configure_python_problem(&problem_dir);
    std::fs::create_dir_all(problem_dir.join("data").join(".cptool-gen.lock")).unwrap();

    let output = run_cptool_allow_failure(
        [
            "pkg",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
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
        ["pkg", "init", "check_json_wait_lock", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("check_json_wait_lock");
    configure_python_problem(&problem_dir);
    run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let handle = release_generation_lock_after(&problem_dir, GENERATION_LOCK_RELEASE_DELAY);

    let output = run_cptool(
        [
            "pkg",
            "check",
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
