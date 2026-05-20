use clap::{Parser, Subcommand};
use cptool::export::{Exporter, OnlineJudge, syzoj};
use cptool::tool::{self, DEFAULT_OUTPUT_LIMIT_BYTES, RunOptions};
use std::path::PathBuf;
use std::time::Instant;

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
        #[arg(help = "Problem id or display name used to create problems/<slug>")]
        id: String,
        #[arg(
            short,
            long,
            default_value = ".",
            help = "Root directory that receives the problems/ folder"
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
            help = "Print only status, size, line count, hash, and stderr summary"
        )]
        summary_only: bool,
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
            help = "Remove stale .in/.ans files for the selected case, bundle, or known bundles before publishing new data"
        )]
        clean: bool,
        #[arg(
            long,
            help = "Print one compact generation summary instead of each generated path"
        )]
        summary_only: bool,
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
            help = "Print one compact summary line per plan instead of per-case progress"
        )]
        summary_only: bool,
    },
    #[command(about = "Check common package structure, config, data, and sample issues")]
    Check {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
    },
    #[command(about = "Export the package to an online judge format")]
    Export {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, value_enum, help = "Target online judge format")]
        oj: OnlineJudge,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { id, root } => {
            let path = tool::init_package(&root, &id)?;
            println!("created {}", path.display());
        }
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
            summary_only,
            hide_stdout,
            args,
        } => {
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
            })?;
            if summary_only {
                println!("{}", result.summary_line());
                if !result.ok {
                    eprintln!(
                        "hint: rerun without --summary-only or use --stdout-path/--stderr-path to save full output"
                    );
                }
            } else {
                println!("{}", result.status_line());
            }
            if !summary_only && !hide_stdout && stdout_path.is_none() && !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !summary_only && stderr_path.is_none() && !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
            if !result.ok {
                std::process::exit(2);
            }
        }
        Commands::Gen {
            work_dir,
            bundle,
            case,
            output_dir,
            output_limit_bytes,
            clean,
            summary_only,
        } => {
            let options = tool::GenerateOptions {
                work_dir,
                bundle,
                selector: case,
                output_dir,
                output_limit_bytes,
                clean,
            };
            if summary_only {
                let report = tool::generate_data_report_with_options(options)?;
                println!("{}", report.summary_line());
            } else {
                let generated = tool::generate_data_with_options(options)?;
                for path in generated {
                    println!("generated {}", path.display());
                }
            }
        }
        Commands::Stress {
            work_dir,
            generator,
            against,
            cases,
            output_limit_bytes,
            failure_dir,
            args,
        } => {
            let summary = tool::stress_with_summary(
                &work_dir,
                &generator,
                &against,
                cases,
                &args,
                failure_dir.as_deref(),
                output_limit_bytes,
            )?;
            println!(
                "stress passed: {} cases unique_input_hashes={}",
                summary.cases, summary.unique_input_hashes
            );
        }
        Commands::StressPlan {
            work_dir,
            name,
            output_limit_bytes,
            failure_dir,
            summary_only,
        } => {
            tool::stress_plan_with_options(tool::StressPlanOptions {
                work_dir: &work_dir,
                name: name.as_deref(),
                failure_dir: failure_dir.as_deref(),
                output_limit_bytes,
                summary_only,
            })?;
        }
        Commands::Check { work_dir } => {
            let report = tool::check_problem_package(&work_dir);
            print!("{}", report.render_text());
            if report.has_errors() {
                std::process::exit(2);
            }
        }
        Commands::Export { work_dir, oj } => {
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
        }
    }
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
