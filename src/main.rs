use clap::{Parser, Subcommand};
use cptool::export::{Exporter, OnlineJudge, syzoj};
use cptool::tool::{self, DEFAULT_OUTPUT_LIMIT_BYTES, RunOptions};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        id: String,
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
    },
    Run {
        program: Option<String>,
        case: Option<String>,
        #[arg(short, long, default_value = ".")]
        work_dir: PathBuf,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        stdin_text: Option<String>,
        #[arg(long)]
        stdin_path: Option<PathBuf>,
        #[arg(long)]
        stdout_path: Option<PathBuf>,
        #[arg(long)]
        stderr_path: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES)]
        output_limit_bytes: usize,
        #[arg(long)]
        summary_only: bool,
        #[arg(long)]
        hide_stdout: bool,
        #[arg(last = true)]
        args: Vec<String>,
    },
    Gen {
        #[arg(short, long, default_value = ".")]
        work_dir: PathBuf,
        #[arg(long)]
        bundle: Option<String>,
        #[arg(long)]
        case: Option<String>,
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES)]
        output_limit_bytes: usize,
        #[arg(long)]
        clean: bool,
    },
    Stress {
        #[arg(short, long, default_value = ".")]
        work_dir: PathBuf,
        #[arg(long)]
        generator: String,
        #[arg(long, required = true)]
        against: Vec<String>,
        #[arg(long, default_value_t = 100)]
        cases: usize,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES)]
        output_limit_bytes: usize,
        #[arg(long)]
        failure_dir: Option<PathBuf>,
        #[arg(last = true)]
        args: Vec<String>,
    },
    StressPlan {
        #[arg(short, long, default_value = ".")]
        work_dir: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES)]
        output_limit_bytes: usize,
        #[arg(long)]
        failure_dir: Option<PathBuf>,
    },
    Check {
        #[arg(short, long, default_value = ".")]
        work_dir: PathBuf,
    },
    Export {
        #[arg(short, long, default_value = ".")]
        work_dir: PathBuf,
        #[arg(long, value_enum)]
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
        } => {
            let generated = tool::generate_data_with_options(tool::GenerateOptions {
                work_dir,
                bundle,
                selector: case,
                output_dir,
                output_limit_bytes,
                clean,
            })?;
            for path in generated {
                println!("generated {}", path.display());
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
            tool::stress(
                &work_dir,
                &generator,
                &against,
                cases,
                &args,
                failure_dir.as_deref(),
                output_limit_bytes,
            )?;
            println!("stress passed: {cases} cases");
        }
        Commands::StressPlan {
            work_dir,
            name,
            output_limit_bytes,
            failure_dir,
        } => {
            tool::stress_plan(
                &work_dir,
                name.as_deref(),
                failure_dir.as_deref(),
                output_limit_bytes,
            )?;
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
