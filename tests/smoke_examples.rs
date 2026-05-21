mod common;
use common::*;
use std::path::PathBuf;

#[test]
fn cli_runs_init_generate_run_stress_and_export_flow() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-full-flow");
    run_cptool(["init", "flow_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("flow_problem");
    configure_python_problem(&problem_dir);

    run_cptool(["gen", "-w"], Some(&problem_dir));

    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.in")).unwrap(),
        "3 4\n"
    );
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.ans")).unwrap(),
        "7\n"
    );

    let stdout_path = problem_dir.join("actual.out");
    run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--stdout-path",
            stdout_path.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(std::fs::read_to_string(&stdout_path).unwrap(), "7\n");

    run_cptool(
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
        "3 4\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.ans")).unwrap(),
        "7\n"
    );
}
#[test]
fn example_problem_packages_generate_and_check() {
    let temp = TempWorkspace::new("cptool-example-packages");
    let example_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example");
    let example_dst = temp.path().join("example");
    copy_example_tree(&example_src, &example_dst);

    let mut checked = Vec::new();
    for entry in std::fs::read_dir(&example_dst).unwrap() {
        let problem_dir = entry.unwrap().path();
        if !problem_dir.join("problem.yaml").is_file() {
            continue;
        }
        let work_dir = problem_dir.to_str().unwrap();
        let gen_output = run_cptool(["gen", "-w", work_dir, "--summary-only"], None);
        let gen_stdout = String::from_utf8_lossy(&gen_output.stdout);
        assert!(gen_stdout.contains("gen: ok"));

        let check = run_cptool(["check", "-w", work_dir], None);
        let check_stdout = String::from_utf8_lossy(&check.stdout);
        assert!(check_stdout.contains("status: `PASS`"));
        checked.push(
            problem_dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
    }

    checked.sort();
    assert_eq!(checked, vec!["a_plus_b".to_string(), "sum".to_string()]);
}
#[test]
fn unicode_paths_and_utf8_data_flow_through_cli() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-unicode 路径");
    run_cptool(["init", "unicode_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("unicode_problem");
    configure_unicode_python_problem(&problem_dir);

    run_cptool(["gen", "-w"], Some(&problem_dir));

    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.in")).unwrap(),
        "你好 世界\n"
    );
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.ans")).unwrap(),
        "答案: 你好 世界\n"
    );

    let stdout_path = problem_dir.join("输出 结果.out");
    run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--stdout-path",
            stdout_path.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(
        std::fs::read_to_string(&stdout_path).unwrap(),
        "答案: 你好 世界\n"
    );

    let check = run_cptool(["check", "-w"], Some(&problem_dir));
    let check_stdout = String::from_utf8_lossy(&check.stdout);
    assert!(check_stdout.contains("status: `PASS`"));

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
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.in")).unwrap(),
        "你好 世界\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.ans")).unwrap(),
        "答案: 你好 世界\n"
    );
}
#[test]
fn cli_help_describes_new_workflow_commands() {
    let version = run_cptool(["--version"], None);
    let version_stdout = String::from_utf8_lossy(&version.stdout);
    assert!(version_stdout.contains(env!("CARGO_PKG_VERSION")));
    assert!(version_stdout.contains("(commit "));

    let top = run_cptool(["--help"], None);
    let top_stdout = String::from_utf8_lossy(&top.stdout);

    assert!(top_stdout.contains("check"));
    assert!(top_stdout.contains("evidence"));
    assert!(top_stdout.contains("stress-plan"));

    let gen_help = run_cptool(["gen", "--help"], None);
    let gen_stdout = String::from_utf8_lossy(&gen_help.stdout);
    assert!(gen_stdout.contains("--clean"));
    assert!(gen_stdout.contains("Remove stale .in/.ans files"));
    assert!(gen_stdout.contains("--summary-only"));
    assert!(gen_stdout.contains("compact generation summary"));
    assert!(gen_stdout.contains("--json"));
    assert!(gen_stdout.contains("--wait-for-generation-lock"));

    let run = run_cptool(["run", "--help"], None);
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run_stdout.contains("--summary-only"));
    assert!(run_stdout.contains("Print only status"));
    assert!(run_stdout.contains("--json"));
    assert!(run_stdout.contains("--wait-for-generation-lock"));

    let check = run_cptool(["check", "--help"], None);
    let check_stdout = String::from_utf8_lossy(&check.stdout);
    assert!(check_stdout.contains("Check common package structure"));
    assert!(check_stdout.contains("--json"));
    assert!(check_stdout.contains("--wait-for-generation-lock"));

    let evidence = run_cptool(["evidence", "--help"], None);
    let evidence_stdout = String::from_utf8_lossy(&evidence.stdout);
    assert!(evidence_stdout.contains("Collect check, generation, and stress-plan evidence"));
    assert!(evidence_stdout.contains("--json"));
    assert!(evidence_stdout.contains("--skip-gen"));
    assert!(evidence_stdout.contains("--reuse-existing-stress-plan"));
    assert!(evidence_stdout.contains("--wait-for-generation-lock"));

    let stress_plan = run_cptool(["stress-plan", "--help"], None);
    let stress_plan_stdout = String::from_utf8_lossy(&stress_plan.stdout);
    assert!(stress_plan_stdout.contains("--name"));
    assert!(stress_plan_stdout.contains("Run only the named stress plan"));
    assert!(stress_plan_stdout.contains("--summary-only"));
    assert!(stress_plan_stdout.contains("--positive-only"));
    assert!(stress_plan_stdout.contains("--negative-only"));
    assert!(stress_plan_stdout.contains("--json"));
    assert!(stress_plan_stdout.contains("--wait-for-generation-lock"));

    let stress = run_cptool(["stress", "--help"], None);
    let stress_stdout = String::from_utf8_lossy(&stress.stdout);
    assert!(stress_stdout.contains("{seed}"));
    assert!(stress_stdout.contains("{case}"));
    assert!(stress_stdout.contains("{case0}"));
    assert!(stress_stdout.contains("--json"));
}
#[test]
fn wait_for_generation_lock_rejects_zero_seconds() {
    for args in [
        &["gen", "--wait-for-generation-lock", "0"][..],
        &["run", "--wait-for-generation-lock", "0"][..],
        &["check", "--wait-for-generation-lock", "0"][..],
        &["stress-plan", "--wait-for-generation-lock", "0"][..],
        &["evidence", "--wait-for-generation-lock", "0"][..],
    ] {
        let output = run_cptool_slice_allow_failure(args, None);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(!output.status.success());
        assert!(stderr.contains("value must be at least 1 second"));
    }
}
