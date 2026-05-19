use clap::{Parser, Subcommand};
use cptool::export::{Exporter, OnlineJudge, syzoj};
use cptool::tool::{self, RunOptions};
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
        #[arg(long, default_value_t = 33_554_432)]
        output_limit_bytes: usize,
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
        #[arg(long, default_value_t = 33_554_432)]
        output_limit_bytes: usize,
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
        #[arg(long, default_value_t = 33_554_432)]
        output_limit_bytes: usize,
        #[arg(long)]
        failure_dir: Option<PathBuf>,
        #[arg(last = true)]
        args: Vec<String>,
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
            println!(
                "{}: {} exit={:?} elapsed={}ms",
                result.label, result.kind, result.exit_code, result.elapsed_ms
            );
            if result.truncated_stdout {
                println!("stdout truncated");
            }
            if result.truncated_stderr {
                println!("stderr truncated");
            }
            if stdout_path.is_none() && !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if stderr_path.is_none() && !result.stderr.is_empty() {
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
        } => {
            let generated = tool::generate_data(
                &work_dir,
                bundle.as_deref(),
                case.as_deref(),
                output_dir.as_deref(),
                output_limit_bytes,
            )?;
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
        Commands::Export { work_dir, oj } => {
            let start = Instant::now();
            let work_dir = if work_dir.is_absolute() {
                work_dir
            } else {
                std::env::current_dir()?.join(work_dir)
            };
            let data_dir = work_dir.join("data");
            let problem = tool::load_problem(&work_dir)?;
            tool::generate_data(&work_dir, None, None, Some(&data_dir), 33_554_432)?;

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
