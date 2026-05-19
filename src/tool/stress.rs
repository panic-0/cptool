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
    reason: String,
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
        anyhow::bail!(
            "{}",
            gen_result.failure_report(&format!("generator failed on stress case {index}"))
        );
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
    Ok(
        classify_stress_failure(&results).map(|reason| StressFailure {
            case_index: index,
            reason,
            input,
            results,
        }),
    )
}

fn save_stress_failure(failure_dir: &Path, failure: StressFailure) -> Result<()> {
    std::fs::create_dir_all(failure_dir)?;
    let (stem, mut input_file) = create_failure_input(failure_dir)?;
    input_file.write_all(&failure.input)?;
    let report = render_stress_failure(failure.case_index, &failure.results);
    std::fs::write(stem.with_extension("txt"), report.as_bytes())?;
    anyhow::bail!(
        "stress failed on case {}; {}; saved {}.in and {}.txt",
        failure.case_index,
        failure.reason,
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
    if let Some(reason) = classify_stress_failure(results) {
        report.push_str(&format!("reason: {reason}\n\n"));
    }
    for result in results {
        report.push_str(&format!("{}\n", result.status_line()));
        report.push_str(&format!("stdout:\n{}\n", result.stdout));
        report.push_str(&format!("stderr:\n{}\n\n", result.stderr));
    }
    report
}

pub(crate) fn classify_stress_failure(results: &[RunResult]) -> Option<String> {
    if let Some(result) = results.iter().find(|result| !result.ok) {
        return Some(format!("program_failed: {}", result.status_line()));
    }

    let baseline = results.first()?;
    let baseline_output = normalize_output(&baseline.stdout);
    results.iter().skip(1).find_map(|result| {
        (normalize_output(&result.stdout) != baseline_output).then(|| {
            format!(
                "wrong_answer: output mismatch between `{}` and `{}`",
                baseline.label, result.label
            )
        })
    })
}
