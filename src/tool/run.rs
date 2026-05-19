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
    write_optional(&options.stdout_path, &result.stdout_bytes)?;
    write_optional(&options.stderr_path, &result.stderr_bytes)?;
    Ok(result)
}
fn write_optional(path: &Option<PathBuf>, content: &[u8]) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::temp_test_dir;

    #[test]
    fn write_optional_preserves_raw_bytes() {
        let root = temp_test_dir("cptool-run-write-test");
        let output_path = root.join("nested").join("stdout.bin");
        let bytes = [0, 0xff, b'\r', b'\n', b'x'];

        write_optional(&Some(output_path.clone()), &bytes).unwrap();

        assert_eq!(std::fs::read(&output_path).unwrap(), bytes);

        std::fs::remove_dir_all(root).unwrap();
    }
}
