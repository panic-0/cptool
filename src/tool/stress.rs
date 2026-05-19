use super::problem::{load_problem, normalize_work_dir, resolve_path};
use super::program::{ProgramSpec, resolve_named_or_source, run_spec};
use super::schema::RunResult;
use anyhow::Result;
use std::io::Write;
use std::path::{Path, PathBuf};
pub fn stress(
    work_dir: &Path,
    generator: &str,
    against: &[String],
    cases: usize,
    args: &[String],
    failure_dir: Option<&Path>,
    output_limit_bytes: usize,
) -> Result<()> {
    if against.len() < 2 {
        anyhow::bail!("stress requires at least two --against programs or sources");
    }
    let work_dir = normalize_work_dir(work_dir)?;
    let problem = load_problem(&work_dir)?;
    let generator = resolve_named_or_source(&work_dir, &problem, generator)?;
    let targets = against
        .iter()
        .map(|item| resolve_named_or_source(&work_dir, &problem, item))
        .collect::<Result<Vec<_>>>()?;
    let failure_dir = failure_dir
        .map(|path| resolve_path(&work_dir, path))
        .unwrap_or_else(|| work_dir.join("tests").join("failures"));

    for index in 1..=cases {
        if let Some(failure) = run_stress_case(
            &work_dir,
            &generator,
            &targets,
            args,
            index,
            output_limit_bytes,
        )? {
            return save_stress_failure(&failure_dir, failure);
        }
        println!("case {index} ok");
    }
    Ok(())
}

struct StressFailure {
    case_index: usize,
    input: Vec<u8>,
    results: Vec<RunResult>,
}

fn run_stress_case(
    work_dir: &Path,
    generator: &ProgramSpec,
    targets: &[ProgramSpec],
    args: &[String],
    index: usize,
    output_limit_bytes: usize,
) -> Result<Option<StressFailure>> {
    let gen_result = run_spec(work_dir, generator, args, None, output_limit_bytes)?;
    if !gen_result.ok {
        anyhow::bail!("generator failed on case {}:\n{}", index, gen_result.stderr);
    }
    if gen_result.truncated_stdout {
        anyhow::bail!(
            "generator output on stress case {index} exceeded --output-limit-bytes ({output_limit_bytes})"
        );
    }
    let input = gen_result.stdout_bytes;
    let mut results = Vec::new();
    for target in targets {
        let result = run_spec(work_dir, target, &[], Some(&input), output_limit_bytes)?;
        if result.truncated_stdout {
            anyhow::bail!(
                "program `{}` output on stress case {index} exceeded --output-limit-bytes ({output_limit_bytes})",
                result.label
            );
        }
        results.push(result);
    }
    let baseline = normalize_output(&results[0].stdout);
    let failed = results
        .iter()
        .any(|result| !result.ok || normalize_output(&result.stdout) != baseline);
    Ok(failed.then_some(StressFailure {
        case_index: index,
        input,
        results,
    }))
}

fn save_stress_failure(failure_dir: &Path, failure: StressFailure) -> Result<()> {
    std::fs::create_dir_all(failure_dir)?;
    let (stem, mut input_file) = create_failure_input(failure_dir)?;
    input_file.write_all(&failure.input)?;
    let report = render_stress_failure(failure.case_index, &failure.results);
    std::fs::write(stem.with_extension("txt"), report.as_bytes())?;
    anyhow::bail!(
        "stress failed on case {}; saved {}.in and {}.txt",
        failure.case_index,
        stem.display(),
        stem.display()
    );
}

fn create_failure_input(failure_dir: &Path) -> Result<(PathBuf, std::fs::File)> {
    for id in 1.. {
        let stem = failure_dir.join(format!("stress-{id:03}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stem.with_extension("in"))
        {
            Ok(file) => return Ok((stem, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!()
}
pub(crate) fn normalize_output(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!(
            "{}\n",
            trimmed
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

fn render_stress_failure(case_index: usize, results: &[RunResult]) -> String {
    let mut report = format!("stress failed on case {case_index}\n\n");
    for result in results {
        report.push_str(&format!(
            "[{}] kind={} exit={:?} elapsed={}ms\nstdout:\n{}\nstderr:\n{}\n\n",
            result.label,
            result.kind,
            result.exit_code,
            result.elapsed_ms,
            result.stdout,
            result.stderr
        ));
    }
    report
}
