use clap::{Parser, Subcommand, ValueEnum};
use cptool::export::OnlineJudge;
use cptool::tool::DEFAULT_OUTPUT_LIMIT_BYTES;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    version = env!("CPTOOL_VERSION"),
    about = "Deterministic competitive-programming problem package tool",
    long_about = "cptool initializes problem packages, runs configured programs, generates official data, stress-tests solutions, checks package health, and exports judge data."
)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Commands,
}

#[derive(Debug, Subcommand)]
pub(super) enum Commands {
    #[command(about = "Create a minimal cptool/autocpp problem package")]
    Init {
        #[arg(help = "Problem id or display name used to create <problems-dir>/<slug>")]
        id: String,
        #[arg(
            short,
            long,
            default_value = ".",
            help = "Workspace root that receives problems/, or the problems/ directory itself"
        )]
        root: PathBuf,
    },
    #[command(about = "Add programs, bundles, tasks, or checkers to problem.yaml")]
    Add {
        #[command(subcommand)]
        command: AddCommands,
    },
    #[command(about = "Run a configured program or source file on package input")]
    Run {
        #[arg(help = "Program name from problem.yaml, or omit to run the configured solution")]
        program: Option<String>,
        #[arg(
            help = "Bundle case selector such as sample[0]; defaults to the first configured case"
        )]
        case: Option<String>,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            help = "Run an explicit .cpp/.py source instead of a configured program"
        )]
        source: Option<PathBuf>,
        #[arg(long, help = "Use this literal text as stdin")]
        stdin_text: Option<String>,
        #[arg(
            long,
            help = "Read stdin from this path, relative to the package when not absolute"
        )]
        stdin_path: Option<PathBuf>,
        #[arg(
            long,
            help = "Write raw stdout bytes to this path instead of printing them"
        )]
        stdout_path: Option<PathBuf>,
        #[arg(
            long,
            help = "Write raw stderr bytes to this path instead of printing them"
        )]
        stderr_path: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_f64,
            help = "Override this run's configured time limit in seconds"
        )]
        time_limit_secs: Option<f64>,
        #[arg(
            long,
            value_name = "MB",
            value_parser = positive_f64,
            help = "Override this run's configured memory limit in megabytes"
        )]
        memory_limit_mb: Option<f64>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(
            long,
            help = "Print only status, size, line count, hash, and stderr summary"
        )]
        summary_only: bool,
        #[arg(
            long,
            help = "Print a JSON run summary; suppresses raw stdout/stderr terminal output"
        )]
        json: bool,
        #[arg(
            long,
            help = "Hide stdout in the terminal while preserving the status line and stderr"
        )]
        hide_stdout: bool,
        #[arg(last = true, help = "Extra arguments passed to the program after --")]
        args: Vec<String>,
    },
    #[command(about = "Run configured validator or checker on local files")]
    Judge {
        #[command(subcommand)]
        command: JudgeCommands,
    },
    #[command(about = "Generate official .in/.ans data from problem.yaml bundles")]
    Gen {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Generate every case in this bundle")]
        bundle: Option<String>,
        #[arg(long, help = "Generate one case selector such as large[0]")]
        case: Option<String>,
        #[arg(short, long, help = "Output directory; defaults to <work-dir>/data")]
        output_dir: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(
            long,
            help = "Remove stale .in/.ans files for the selected case, bundle, or known bundles before publishing new data"
        )]
        clean: bool,
        #[arg(
            long,
            help = "Print one compact generation summary instead of each generated path"
        )]
        summary_only: bool,
        #[arg(long, help = "Print the generation report as JSON")]
        json: bool,
    },
    #[command(about = "Clean generated data and local cptool cache")]
    Clean {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Clean generated data files from data/")]
        data: bool,
        #[arg(long, help = "Clean local cptool cache from .cptool/cache")]
        cache: bool,
        #[arg(long, help = "Print the clean report as JSON")]
        json: bool,
    },
    #[command(
        about = "Stress test several programs on temporary generated inputs",
        long_about = "Stress test several programs on temporary generated inputs. Generator args after -- support {seed}, {case}, and {case0}; {case} is 1-based, {case0} is 0-based, and {seed} is deterministic."
    )]
    Stress {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Generator program name from problem.yaml or source path")]
        generator: String,
        #[arg(
            long,
            required = true,
            help = "Program name or source path to compare; pass at least two"
        )]
        against: Vec<String>,
        #[arg(
            long,
            default_value_t = 100,
            help = "Number of generated cases to test"
        )]
        cases: usize,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Directory for failed inputs and per-program outputs")]
        failure_dir: Option<PathBuf>,
        #[arg(long, help = "Print the stress summary as JSON")]
        json: bool,
        #[arg(
            last = true,
            help = "Arguments passed to the generator after --; supports {seed}, {case}, and {case0}"
        )]
        args: Vec<String>,
    },
    #[command(
        about = "Run stress plans declared in problem.yaml",
        long_about = "Run stress plans declared in problem.yaml. Plan args support {seed}, {case}, and {case0}; {seed} is deterministic and may be controlled with stress.plans[].seed_base."
    )]
    StressPlan {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Run only the named stress plan; omit to run all plans")]
        name: Option<String>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Directory for failed inputs and per-program outputs")]
        failure_dir: Option<PathBuf>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(
            long,
            help = "Print one compact summary line per plan instead of per-case progress"
        )]
        summary_only: bool,
        #[arg(
            long,
            conflicts_with = "negative_only",
            help = "Run only expect: pass plans"
        )]
        positive_only: bool,
        #[arg(
            long,
            conflicts_with = "positive_only",
            help = "Run only expect: fail plans"
        )]
        negative_only: bool,
        #[arg(long, help = "Print stress plan summaries as JSON")]
        json: bool,
    },
    #[command(about = "Check common package structure, config, data, and sample issues")]
    Check {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(long, help = "Print the check report as JSON")]
        json: bool,
    },
    #[command(about = "Collect check, generation, and stress-plan evidence")]
    Evidence {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Skip official data generation evidence")]
        skip_gen: bool,
        #[arg(long, help = "Skip stress-plan evidence")]
        skip_stress_plan: bool,
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with = "skip_stress_plan",
            help = "Reuse JSON from `stress-plan --summary-only --json` instead of rerunning stress plans"
        )]
        reuse_existing_stress_plan: Option<PathBuf>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(long, help = "Print the evidence report as JSON")]
        json: bool,
        #[arg(
            long,
            conflicts_with = "json",
            help = "Print a quality_report.md-ready Markdown evidence section"
        )]
        markdown: bool,
    },
    #[command(about = "Export the package to an online judge format")]
    Export {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, value_enum, help = "Target online judge format")]
        oj: OnlineJudge,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum AddCommands {
    #[command(about = "Register a program, auto-creating src/<name>.cpp when needed")]
    Program {
        #[arg(help = "Program key to add under problem.yaml programs")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            value_enum,
            help = "Program kind; inferred from path when omitted"
        )]
        kind: Option<AddProgramKindArg>,
        #[arg(long, help = "Program path; defaults to src/<name>.* auto-detection")]
        path: Option<PathBuf>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_f64,
            help = "Configured program time limit in seconds"
        )]
        time_limit_secs: Option<f64>,
        #[arg(
            long,
            value_name = "MB",
            value_parser = positive_f64,
            help = "Configured program memory limit in megabytes"
        )]
        memory_limit_mb: Option<f64>,
        #[arg(
            long,
            value_name = "ARG",
            help = "C++ compile arg; replaces defaults when present"
        )]
        compile_arg: Vec<String>,
        #[arg(long, help = "Replace an existing program entry")]
        replace: bool,
    },
    #[command(about = "Register a test bundle")]
    Bundle {
        #[arg(help = "Bundle name to add under test.bundles")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Generator program for every added case; defaults to gen")]
        generator: Option<String>,
        #[arg(
            long = "case",
            required = true,
            help = "Comma-separated generator args; use an empty string for []"
        )]
        cases: Vec<String>,
        #[arg(long, help = "Replace an existing bundle")]
        replace: bool,
    },
    #[command(about = "Register a test task")]
    Task {
        #[arg(help = "Task name to add under test.tasks")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Task score")]
        score: f64,
        #[arg(long = "type", value_enum, default_value_t = AddTaskTypeArg::Min, help = "Task scoring aggregation type")]
        task_type: AddTaskTypeArg,
        #[arg(
            long = "bundle",
            required = true,
            help = "Bundle included in this task"
        )]
        bundles: Vec<String>,
        #[arg(long = "depends", help = "Task dependency")]
        dependencies: Vec<String>,
        #[arg(long, help = "Replace an existing task")]
        replace: bool,
    },
    #[command(about = "Register a checker, copying a built-in testlib checker into src/")]
    Checker {
        #[arg(default_value = "chk", help = "Checker program key; defaults to chk")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Built-in testlib checker id to copy, e.g. wcmp")]
        builtin: String,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_f64,
            help = "Configured checker time limit in seconds"
        )]
        time_limit_secs: Option<f64>,
        #[arg(
            long,
            value_name = "MB",
            value_parser = positive_f64,
            help = "Configured checker memory limit in megabytes"
        )]
        memory_limit_mb: Option<f64>,
        #[arg(
            long,
            value_name = "ARG",
            help = "C++ compile arg; replaces defaults when present"
        )]
        compile_arg: Vec<String>,
        #[arg(long, help = "Replace existing checker config or source")]
        replace: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum JudgeCommands {
    #[command(about = "Run the configured validator on an input file")]
    Validator {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            help = "Validator program key; defaults to problem.yaml validator"
        )]
        validator: Option<String>,
        #[arg(long, value_name = "PATH", help = "Input file to validate")]
        input_path: PathBuf,
        #[arg(long, value_enum, default_value_t = JudgeExpectationArg::Pass, help = "Expected verdict")]
        expect: JudgeExpectationArg,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Print the judge result as JSON")]
        json: bool,
    },
    #[command(about = "Run the configured checker on input/output/answer files")]
    Checker {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Checker program key; defaults to problem.yaml checker")]
        checker: Option<String>,
        #[arg(long, value_name = "PATH", help = "Input file passed to the checker")]
        input_path: PathBuf,
        #[arg(
            long,
            value_name = "PATH",
            help = "Participant output file passed to the checker"
        )]
        output_path: PathBuf,
        #[arg(
            long,
            value_name = "PATH",
            help = "Jury answer file passed to the checker"
        )]
        answer_path: PathBuf,
        #[arg(long, value_enum, default_value_t = JudgeExpectationArg::Pass, help = "Expected verdict")]
        expect: JudgeExpectationArg,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Print the judge result as JSON")]
        json: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum AddProgramKindArg {
    Cpp,
    Python,
    Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum AddTaskTypeArg {
    Min,
    Sum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum JudgeExpectationArg {
    Pass,
    Fail,
}

pub(super) fn positive_seconds(value: &str) -> Result<u64, String> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| format!("`{value}` is not a positive integer number of seconds"))?;
    if seconds == 0 {
        return Err("value must be at least 1 second".to_string());
    }
    Ok(seconds)
}

pub(super) fn positive_f64(value: &str) -> Result<f64, String> {
    let number = value
        .parse::<f64>()
        .map_err(|_| format!("`{value}` is not a positive finite number"))?;
    if !number.is_finite() || number <= 0.0 {
        return Err("value must be a positive finite number".to_string());
    }
    Ok(number)
}
