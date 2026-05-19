use super::problem::{
    case_file_stem, load_problem, normalize_work_dir, parse_case_selector, resolve_path,
};
use super::program::{ProgramSpec, absolutize_program_info, run_spec};
use super::schema::{CaseSelector, Problem};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct GenerateContext<'a> {
    work_dir: &'a Path,
    problem: &'a Problem,
    programs: &'a HashMap<String, ProgramSpec>,
    solution: &'a ProgramSpec,
    validator: Option<&'a ProgramSpec>,
    output_dir: &'a Path,
    output_limit_bytes: usize,
}

pub fn generate_data(
    work_dir: &Path,
    bundle: Option<&str>,
    selector: Option<&str>,
    output_dir: Option<&Path>,
    output_limit_bytes: usize,
) -> Result<Vec<PathBuf>> {
    let work_dir = normalize_work_dir(work_dir)?;
    let problem = load_problem(&work_dir)?;
    let output_dir = output_dir
        .map(|path| resolve_path(&work_dir, path))
        .unwrap_or_else(|| work_dir.join("data"));
    std::fs::create_dir_all(&output_dir)?;
    let programs = compile_programs(&work_dir, &problem)?;
    let solution = programs
        .get(&problem.solution_name)
        .with_context(|| format!("solution `{}` not found", problem.solution_name))?;
    let validator = problem
        .validator_name
        .as_ref()
        .map(|name| {
            programs
                .get(name)
                .with_context(|| format!("validator `{name}` not found"))
        })
        .transpose()?;
    let context = GenerateContext {
        work_dir: &work_dir,
        problem: &problem,
        programs: &programs,
        solution,
        validator,
        output_dir: &output_dir,
        output_limit_bytes,
    };

    let mut generated = Vec::new();
    if let Some(selector) = selector {
        let selector = parse_case_selector(selector)?;
        generated.extend(generate_one_case(&context, &selector)?);
    } else if let Some(bundle) = bundle {
        let bundle_cases = problem
            .test
            .bundles
            .get(bundle)
            .with_context(|| format!("bundle `{bundle}` not found"))?;
        for index in 0..bundle_cases.cases.len() {
            generated.extend(generate_one_case(
                &context,
                &CaseSelector {
                    bundle: bundle.to_string(),
                    index,
                },
            )?);
        }
    } else {
        for (bundle, bundle_cases) in &problem.test.bundles {
            for index in 0..bundle_cases.cases.len() {
                generated.extend(generate_one_case(
                    &context,
                    &CaseSelector {
                        bundle: bundle.clone(),
                        index,
                    },
                )?);
            }
        }
    }
    Ok(generated)
}
fn compile_programs(work_dir: &Path, problem: &Problem) -> Result<HashMap<String, ProgramSpec>> {
    problem
        .programs
        .iter()
        .map(|(name, program)| {
            Ok((
                name.clone(),
                ProgramSpec {
                    label: name.clone(),
                    info: absolutize_program_info(work_dir, &program.info),
                    time_limit_secs: program.time_limit_secs,
                    memory_limit_mb: program.memory_limit_mb,
                },
            ))
        })
        .collect()
}

fn generate_one_case(
    context: &GenerateContext<'_>,
    selector: &CaseSelector,
) -> Result<Vec<PathBuf>> {
    let bundle = context
        .problem
        .test
        .bundles
        .get(&selector.bundle)
        .with_context(|| format!("bundle `{}` not found", selector.bundle))?;
    let case = bundle
        .cases
        .get(selector.index)
        .with_context(|| format!("case `{}` not found", selector.index))?;
    let generator = context
        .programs
        .get(&case.generator_name)
        .with_context(|| format!("generator `{}` not found", case.generator_name))?;
    let stem = context.output_dir.join(case_file_stem(selector));
    let input_path = stem.with_extension("in");
    let answer_path = stem.with_extension("ans");
    let generated = run_spec(
        context.work_dir,
        generator,
        &case.args,
        None,
        context.output_limit_bytes,
    )?;
    if !generated.ok {
        anyhow::bail!(
            "generator failed for {}[{}]:\n{}",
            selector.bundle,
            selector.index,
            generated.stderr
        );
    }
    if generated.truncated_stdout {
        anyhow::bail!(
            "generator output for {}[{}] exceeded --output-limit-bytes ({})",
            selector.bundle,
            selector.index,
            context.output_limit_bytes
        );
    }
    std::fs::write(&input_path, &generated.stdout_bytes)?;
    if let Some(validator) = context.validator {
        let validation = run_spec(
            context.work_dir,
            validator,
            &[],
            Some(&generated.stdout_bytes),
            context.output_limit_bytes,
        )?;
        if !validation.ok {
            anyhow::bail!(
                "validator failed for {}:\n{}",
                input_path.display(),
                validation.stderr
            );
        }
    }
    let answer = run_spec(
        context.work_dir,
        context.solution,
        &[],
        Some(&generated.stdout_bytes),
        context.output_limit_bytes,
    )?;
    if !answer.ok {
        anyhow::bail!(
            "solution failed for {}:\n{}",
            input_path.display(),
            answer.stderr
        );
    }
    if answer.truncated_stdout {
        anyhow::bail!(
            "solution output for {}[{}] exceeded --output-limit-bytes ({})",
            selector.bundle,
            selector.index,
            context.output_limit_bytes
        );
    }
    std::fs::write(&answer_path, &answer.stdout_bytes)?;
    Ok(vec![input_path, answer_path])
}
