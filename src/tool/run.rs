use super::problem::{load_problem, normalize_work_dir, resolve_run_input};
use super::program::{resolve_run_spec, run_spec};
use super::schema::{RunOptions, RunResult};
use anyhow::Result;
use std::path::PathBuf;
pub fn run(options: RunOptions) -> Result<RunResult> {
    if options.stdin_text.is_some() && options.stdin_path.is_some() {
        anyhow::bail!("use either --stdin-text or --stdin-path, not both");
    }
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let problem = load_problem(&work_dir)?;
    let spec = resolve_run_spec(
        &work_dir,
        &problem,
        options.program.as_deref(),
        options.source.as_deref(),
    )?;
    let input = resolve_run_input(
        &work_dir,
        &problem,
        options.selector.as_deref(),
        options.stdin_text,
        options.stdin_path,
    )?;
    let result = run_spec(
        &work_dir,
        &spec,
        &options.args,
        input.as_deref(),
        options.output_limit_bytes,
    )?;
    write_optional(&options.stdout_path, &result.stdout)?;
    write_optional(&options.stderr_path, &result.stderr)?;
    Ok(result)
}
fn write_optional(path: &Option<PathBuf>, content: &str) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content.as_bytes())?;
    }
    Ok(())
}
