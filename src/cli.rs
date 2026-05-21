use clap::{Parser, Subcommand};
use cptool::export::{Exporter, OnlineJudge, syzoj};
use cptool::tool::{self, DEFAULT_OUTPUT_LIMIT_BYTES, RunOptions};
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod json;

#[derive(Debug, Parser)]
#[command(
    version = env!("CPTOOL_VERSION"),
    about = "Deterministic competitive-programming problem package tool",
    long_about = "cptool initializes problem packages, runs configured programs, generates official data, stress-tests solutions, checks package health, and exports judge data."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
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

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { id, root } => handle_init(id, root)?,
        Commands::Run {
            program,
            case,
            work_dir,
            source,
            stdin_text,
            stdin_path,
            stdout_path,
            stderr_path,
            output_limit_bytes,
            time_limit_secs,
            memory_limit_mb,
            wait_for_generation_lock,
            summary_only,
            json,
            hide_stdout,
            args,
        } => handle_run(RunCommandOptions {
            program,
            case,
            work_dir,
            source,
            stdin_text,
            stdin_path,
            stdout_path,
            stderr_path,
            output_limit_bytes,
            time_limit_secs,
            memory_limit_mb,
            wait_for_generation_lock,
            summary_only,
            json,
            hide_stdout,
            args,
        })?,
        Commands::Gen {
            work_dir,
            bundle,
            case,
            output_dir,
            output_limit_bytes,
            wait_for_generation_lock,
            clean,
            summary_only,
            json,
        } => handle_gen(GenCommandOptions {
            work_dir,
            bundle,
            case,
            output_dir,
            output_limit_bytes,
            wait_for_generation_lock,
            clean,
            summary_only,
            json,
        })?,
        Commands::Clean {
            work_dir,
            data,
            cache,
            json,
        } => handle_clean(work_dir, data, cache, json)?,
        Commands::Stress {
            work_dir,
            generator,
            against,
            cases,
            output_limit_bytes,
            failure_dir,
            json,
            args,
        } => handle_stress(StressCommandOptions {
            work_dir,
            generator,
            against,
            cases,
            output_limit_bytes,
            failure_dir,
            json,
            args,
        })?,
        Commands::StressPlan {
            work_dir,
            name,
            output_limit_bytes,
            failure_dir,
            wait_for_generation_lock,
            summary_only,
            positive_only,
            negative_only,
            json,
        } => handle_stress_plan(StressPlanCommandOptions {
            work_dir,
            name,
            output_limit_bytes,
            failure_dir,
            wait_for_generation_lock,
            summary_only,
            positive_only,
            negative_only,
            json,
        })?,
        Commands::Check {
            work_dir,
            wait_for_generation_lock,
            json,
        } => handle_check(work_dir, wait_for_generation_lock, json)?,
        Commands::Evidence {
            work_dir,
            output_limit_bytes,
            skip_gen,
            skip_stress_plan,
            reuse_existing_stress_plan,
            wait_for_generation_lock,
            json,
            markdown,
        } => handle_evidence(EvidenceCommandOptions {
            work_dir,
            output_limit_bytes,
            skip_gen,
            skip_stress_plan,
            reuse_existing_stress_plan,
            wait_for_generation_lock,
            json,
            markdown,
        })?,
        Commands::Export { work_dir, oj } => handle_export(work_dir, oj)?,
    }
    Ok(())
}

fn handle_init(id: String, root: PathBuf) -> anyhow::Result<()> {
    let path = tool::init_package(&root, &id)?;
    println!("created {}", path.display());
    Ok(())
}

struct RunCommandOptions {
    program: Option<String>,
    case: Option<String>,
    work_dir: PathBuf,
    source: Option<PathBuf>,
    stdin_text: Option<String>,
    stdin_path: Option<PathBuf>,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
    output_limit_bytes: usize,
    time_limit_secs: Option<f64>,
    memory_limit_mb: Option<f64>,
    wait_for_generation_lock: Option<u64>,
    summary_only: bool,
    json: bool,
    hide_stdout: bool,
    args: Vec<String>,
}

fn handle_run(options: RunCommandOptions) -> anyhow::Result<()> {
    let RunCommandOptions {
        program,
        case,
        work_dir,
        source,
        stdin_text,
        stdin_path,
        stdout_path,
        stderr_path,
        output_limit_bytes,
        time_limit_secs,
        memory_limit_mb,
        wait_for_generation_lock,
        summary_only,
        json,
        hide_stdout,
        args,
    } = options;
    let (program, selector) = normalize_run_positionals(program, case);
    let result = tool::run(RunOptions {
        work_dir,
        program,
        source,
        selector,
        stdin_text,
        stdin_path,
        stdout_path: stdout_path.clone(),
        stderr_path: stderr_path.clone(),
        args,
        output_limit_bytes,
        generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
        time_limit_secs,
        memory_limit_mb,
    })?;
    if json {
        self::json::print(&self::json::RunJsonSummary::from(&result))?;
    } else if summary_only {
        println!("{}", result.summary_line());
        if !result.ok {
            eprintln!(
                "hint: rerun without --summary-only or use --stdout-path/--stderr-path to save full output"
            );
        }
    } else {
        println!("{}", result.status_line());
    }
    if !json && !summary_only && !hide_stdout && stdout_path.is_none() && !result.stdout.is_empty()
    {
        print!("{}", result.stdout);
    }
    if !json && !summary_only && stderr_path.is_none() && !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }
    if !result.ok {
        std::process::exit(2);
    }
    Ok(())
}

struct GenCommandOptions {
    work_dir: PathBuf,
    bundle: Option<String>,
    case: Option<String>,
    output_dir: Option<PathBuf>,
    output_limit_bytes: usize,
    wait_for_generation_lock: Option<u64>,
    clean: bool,
    summary_only: bool,
    json: bool,
}

fn handle_gen(options: GenCommandOptions) -> anyhow::Result<()> {
    let GenCommandOptions {
        work_dir,
        bundle,
        case,
        output_dir,
        output_limit_bytes,
        wait_for_generation_lock,
        clean,
        summary_only,
        json,
    } = options;
    let options = tool::GenerateOptions {
        work_dir,
        bundle,
        selector: case,
        output_dir,
        output_limit_bytes,
        clean,
        generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
    };
    if json {
        let report = tool::generate_data_report_with_options(options)?;
        self::json::print(&report)?;
    } else if summary_only {
        let report = tool::generate_data_report_with_options(options)?;
        println!("{}", report.summary_line());
    } else {
        let generated = tool::generate_data_with_options(options)?;
        for path in generated {
            println!("generated {}", path.display());
        }
    }
    Ok(())
}

fn handle_clean(work_dir: PathBuf, data: bool, cache: bool, json: bool) -> anyhow::Result<()> {
    let report = tool::clean_package_with_options(tool::CleanOptions {
        work_dir,
        data,
        cache,
    })?;
    if json {
        self::json::print(&report)?;
    } else {
        println!("{}", report.summary_line());
    }
    Ok(())
}

struct StressCommandOptions {
    work_dir: PathBuf,
    generator: String,
    against: Vec<String>,
    cases: usize,
    output_limit_bytes: usize,
    failure_dir: Option<PathBuf>,
    json: bool,
    args: Vec<String>,
}

fn handle_stress(options: StressCommandOptions) -> anyhow::Result<()> {
    if options.json {
        let summary = tool::stress_with_options(tool::StressOptions {
            work_dir: &options.work_dir,
            generator: &options.generator,
            against: &options.against,
            cases: options.cases,
            args: &options.args,
            failure_dir: options.failure_dir.as_deref(),
            output_limit_bytes: options.output_limit_bytes,
            print_progress: false,
            print_warnings: false,
        })?;
        self::json::print(&self::json::StressJsonSummary::from(&summary))?;
    } else {
        let summary = tool::stress_with_summary(
            &options.work_dir,
            &options.generator,
            &options.against,
            options.cases,
            &options.args,
            options.failure_dir.as_deref(),
            options.output_limit_bytes,
        )?;
        println!(
            "stress passed: {} cases unique_input_hashes={}",
            summary.cases, summary.unique_input_hashes
        );
    }
    Ok(())
}

struct StressPlanCommandOptions {
    work_dir: PathBuf,
    name: Option<String>,
    output_limit_bytes: usize,
    failure_dir: Option<PathBuf>,
    wait_for_generation_lock: Option<u64>,
    summary_only: bool,
    positive_only: bool,
    negative_only: bool,
    json: bool,
}

fn handle_stress_plan(options: StressPlanCommandOptions) -> anyhow::Result<()> {
    let stress_options = tool::StressPlanOptions {
        work_dir: &options.work_dir,
        name: options.name.as_deref(),
        failure_dir: options.failure_dir.as_deref(),
        output_limit_bytes: options.output_limit_bytes,
        summary_only: options.summary_only,
        filter: stress_plan_filter(options.positive_only, options.negative_only),
        generation_lock_timeout: generation_lock_timeout(options.wait_for_generation_lock),
    };
    if options.json {
        let summaries = tool::stress_plan_collect_with_options(stress_options)?;
        let report = self::json::StressPlanJsonReport::from_summaries(&summaries);
        self::json::print(&report)?;
    } else {
        tool::stress_plan_with_options(stress_options)?;
    }
    Ok(())
}

fn handle_check(
    work_dir: PathBuf,
    wait_for_generation_lock: Option<u64>,
    json: bool,
) -> anyhow::Result<()> {
    let report = tool::check_problem_package_with_options(
        &work_dir,
        tool::CheckOptions {
            generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
        },
    );
    if json {
        self::json::print(&self::json::CheckJsonReport::from(&report))?;
    } else {
        print!("{}", report.render_text());
    }
    if report.has_errors() {
        std::process::exit(2);
    }
    Ok(())
}

struct EvidenceCommandOptions {
    work_dir: PathBuf,
    output_limit_bytes: usize,
    skip_gen: bool,
    skip_stress_plan: bool,
    reuse_existing_stress_plan: Option<PathBuf>,
    wait_for_generation_lock: Option<u64>,
    json: bool,
    markdown: bool,
}

fn handle_evidence(options: EvidenceCommandOptions) -> anyhow::Result<()> {
    let report = tool::collect_evidence(tool::EvidenceOptions {
        work_dir: options.work_dir,
        output_limit_bytes: options.output_limit_bytes,
        skip_gen: options.skip_gen,
        skip_stress_plan: options.skip_stress_plan,
        reuse_existing_stress_plan: options.reuse_existing_stress_plan,
        generation_lock_timeout: generation_lock_timeout(options.wait_for_generation_lock),
    });
    if options.json {
        self::json::print(&report)?;
    } else if options.markdown {
        print!("{}", report.render_quality_markdown());
    } else {
        print!("{}", report.render_text());
    }
    if report.has_errors() {
        std::process::exit(2);
    }
    Ok(())
}

fn handle_export(work_dir: PathBuf, oj: OnlineJudge) -> anyhow::Result<()> {
    let start = Instant::now();
    let work_dir = if work_dir.is_absolute() {
        work_dir
    } else {
        std::env::current_dir()?.join(work_dir)
    };
    let data_dir = work_dir.join("data");
    let problem = tool::load_problem(&work_dir)?;
    tool::generate_data(
        &work_dir,
        None,
        None,
        Some(&data_dir),
        DEFAULT_OUTPUT_LIMIT_BYTES,
    )?;

    match oj {
        OnlineJudge::Syzoj => {
            let export_dir = work_dir.join("export").join("syzoj");
            if export_dir.exists() {
                std::fs::remove_dir_all(&export_dir)?;
            }
            std::fs::create_dir_all(&export_dir)?;
            syzoj::SyzojExporter::export(&problem, &work_dir, &data_dir, &export_dir)?;
            println!("exported {}", export_dir.display());
        }
    }
    let elapsed = start.elapsed();
    println!(
        "elapsed: {}.{:03}s",
        elapsed.as_secs(),
        elapsed.subsec_millis()
    );
    Ok(())
}

fn normalize_run_positionals(
    program: Option<String>,
    case: Option<String>,
) -> (Option<String>, Option<String>) {
    match (program, case) {
        (Some(first), None) if first.contains('[') && first.ends_with(']') => (None, Some(first)),
        (program, case) => (program, case),
    }
}

fn stress_plan_filter(positive_only: bool, negative_only: bool) -> tool::StressPlanFilter {
    if positive_only {
        tool::StressPlanFilter::PositiveOnly
    } else if negative_only {
        tool::StressPlanFilter::NegativeOnly
    } else {
        tool::StressPlanFilter::All
    }
}

fn positive_seconds(value: &str) -> Result<u64, String> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| format!("`{value}` is not a positive integer number of seconds"))?;
    if seconds == 0 {
        return Err("value must be at least 1 second".to_string());
    }
    Ok(seconds)
}

fn positive_f64(value: &str) -> Result<f64, String> {
    let number = value
        .parse::<f64>()
        .map_err(|_| format!("`{value}` is not a positive finite number"))?;
    if !number.is_finite() || number <= 0.0 {
        return Err("value must be a positive finite number".to_string());
    }
    Ok(number)
}

fn generation_lock_timeout(seconds: Option<u64>) -> Option<Duration> {
    seconds.map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cptool::support::count_lines;

    #[test]
    fn normalizes_single_case_selector_positional() {
        assert_eq!(
            normalize_run_positionals(Some("sample[0]".to_string()), None),
            (None, Some("sample[0]".to_string()))
        );
        assert_eq!(
            normalize_run_positionals(Some("solution".to_string()), None),
            (Some("solution".to_string()), None)
        );
        assert_eq!(
            normalize_run_positionals(Some("solution".to_string()), Some("sample[0]".to_string())),
            (Some("solution".to_string()), Some("sample[0]".to_string()))
        );
    }

    #[test]
    fn positive_seconds_rejects_zero_and_non_numbers() {
        assert_eq!(positive_seconds("1"), Ok(1));
        assert!(positive_seconds("0").is_err());
        assert!(positive_seconds("1.5").is_err());
        assert!(positive_seconds("abc").is_err());
    }

    #[test]
    fn positive_f64_rejects_non_positive_and_non_finite_values() {
        assert_eq!(positive_f64("1"), Ok(1.0));
        assert_eq!(positive_f64("0.25"), Ok(0.25));
        assert!(positive_f64("0").is_err());
        assert!(positive_f64("-1").is_err());
        assert!(positive_f64("NaN").is_err());
        assert!(positive_f64("inf").is_err());
        assert!(positive_f64("abc").is_err());
    }

    #[test]
    fn generation_lock_timeout_maps_seconds_to_duration() {
        assert_eq!(generation_lock_timeout(None), None);
        assert_eq!(
            generation_lock_timeout(Some(3)),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn stress_plan_filter_prefers_positive_when_both_are_set() {
        assert_eq!(
            stress_plan_filter(false, false),
            tool::StressPlanFilter::All
        );
        assert_eq!(
            stress_plan_filter(true, false),
            tool::StressPlanFilter::PositiveOnly
        );
        assert_eq!(
            stress_plan_filter(false, true),
            tool::StressPlanFilter::NegativeOnly
        );
        assert_eq!(
            stress_plan_filter(true, true),
            tool::StressPlanFilter::PositiveOnly
        );
    }

    #[test]
    fn count_lines_handles_trailing_newlines() {
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"one"), 1);
        assert_eq!(count_lines(b"one\n"), 1);
        assert_eq!(count_lines(b"one\ntwo"), 2);
        assert_eq!(count_lines(b"one\ntwo\n"), 2);
    }
}
