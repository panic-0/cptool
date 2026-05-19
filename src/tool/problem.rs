use super::data::generate_data;
use super::schema::{CaseSelector, DEFAULT_OUTPUT_LIMIT_BYTES, Problem};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
pub fn load_problem(work_dir: &Path) -> Result<Problem> {
    let path = work_dir.join("problem.yaml");
    let yaml = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let problem: Problem = serde_yml::from_str(&yaml)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_problem(&problem).with_context(|| format!("invalid {}", path.display()))?;
    Ok(problem)
}

pub fn parse_case_selector(value: &str) -> Result<CaseSelector> {
    let Some(open) = value.rfind('[') else {
        anyhow::bail!("case selector must look like bundle[index], got `{value}`");
    };
    if !value.ends_with(']') {
        anyhow::bail!("case selector must look like bundle[index], got `{value}`");
    }
    let bundle = value[..open].to_string();
    let raw_index = &value[open + 1..value.len() - 1];
    if bundle.is_empty() {
        anyhow::bail!("case selector bundle cannot be empty");
    }
    let index = raw_index
        .parse::<usize>()
        .with_context(|| format!("invalid case selector index `{raw_index}`"))?;
    Ok(CaseSelector { bundle, index })
}
pub(crate) fn resolve_run_input(
    work_dir: &Path,
    problem: &Problem,
    selector: Option<&str>,
    stdin_text: Option<String>,
    stdin_path: Option<PathBuf>,
) -> Result<Option<Vec<u8>>> {
    if let Some(text) = stdin_text {
        return Ok(Some(text.into_bytes()));
    }
    if let Some(path) = stdin_path {
        return Ok(Some(std::fs::read(resolve_path(work_dir, &path))?));
    }
    let selector = match selector {
        Some(selector) => parse_case_selector(selector)?,
        None => default_selector(problem)?,
    };
    let input_path = work_dir
        .join("data")
        .join(case_file_stem(&selector))
        .with_extension("in");
    if !input_path.exists() {
        generate_data(
            work_dir,
            Some(&selector.bundle),
            Some(&format!("{}[{}]", selector.bundle, selector.index)),
            None,
            DEFAULT_OUTPUT_LIMIT_BYTES,
        )?;
    }
    Ok(Some(std::fs::read(&input_path).with_context(|| {
        format!("failed to read {}", input_path.display())
    })?))
}

fn default_selector(problem: &Problem) -> Result<CaseSelector> {
    if let Some(task) = problem.test.tasks.first() {
        if let Some(bundle) = task.bundles.first() {
            return Ok(CaseSelector {
                bundle: bundle.clone(),
                index: 0,
            });
        }
    }
    let Some(bundle) = problem.test.bundles.keys().min().cloned() else {
        anyhow::bail!("problem.yaml has no test bundles");
    };
    Ok(CaseSelector { bundle, index: 0 })
}
pub(crate) fn resolve_path(work_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        work_dir.join(path)
    }
}

pub(crate) fn normalize_work_dir(work_dir: &Path) -> Result<PathBuf> {
    if work_dir.is_absolute() {
        Ok(work_dir.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(work_dir))
    }
}

pub(crate) fn case_file_stem(selector: &CaseSelector) -> String {
    format!("{}-{}", selector.bundle, selector.index)
}

fn validate_problem(problem: &Problem) -> Result<()> {
    if problem.programs.is_empty() {
        anyhow::bail!("programs cannot be empty");
    }
    for (name, program) in &problem.programs {
        validate_positive_finite(
            program.time_limit_secs,
            &format!("program `{name}` time_limit_secs"),
        )?;
        validate_positive_finite(
            program.memory_limit_mb,
            &format!("program `{name}` memory_limit_mb"),
        )?;
    }
    ensure_program_exists(problem, &problem.solution_name, "solution")?;
    if let Some(name) = &problem.validator_name {
        ensure_program_exists(problem, name, "validator")?;
    }
    if let Some(name) = &problem.checker_name {
        ensure_program_exists(problem, name, "checker")?;
    }

    for (bundle_name, bundle) in &problem.test.bundles {
        for (case_index, case) in bundle.cases.iter().enumerate() {
            ensure_program_exists(
                problem,
                &case.generator_name,
                &format!("generator for {bundle_name}[{case_index}]"),
            )?;
        }
    }

    let task_names = problem
        .test
        .tasks
        .iter()
        .map(|task| task.name.as_str())
        .collect::<HashSet<_>>();
    for task in &problem.test.tasks {
        validate_non_negative_finite(task.score, &format!("task `{}` score", task.name))?;
        for bundle_name in &task.bundles {
            if !problem.test.bundles.contains_key(bundle_name) {
                anyhow::bail!(
                    "task `{}` references missing bundle `{bundle_name}`",
                    task.name
                );
            }
        }
        for dependency in &task.dependencies {
            if !task_names.contains(dependency.as_str()) {
                anyhow::bail!(
                    "task `{}` references missing dependency `{dependency}`",
                    task.name
                );
            }
        }
    }
    Ok(())
}

fn ensure_program_exists(problem: &Problem, name: &str, role: &str) -> Result<()> {
    if !problem.programs.contains_key(name) {
        anyhow::bail!("{role} `{name}` is not defined in programs");
    }
    Ok(())
}

fn validate_positive_finite(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        anyhow::bail!("{label} must be a positive finite number");
    }
    Ok(())
}

fn validate_non_negative_finite(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        anyhow::bail!("{label} must be a non-negative finite number");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::schema::{
        CppProgram, Program, ProgramInfo, Test, TestBundle, TestCase, TestTask, TestTaskType,
    };
    use std::collections::HashMap;

    #[test]
    fn validate_problem_rejects_missing_program_references() {
        let mut problem = valid_problem();
        problem.solution_name = "missing".to_string();

        let err = validate_problem(&problem).unwrap_err().to_string();

        assert!(err.contains("solution `missing`"));
    }

    #[test]
    fn validate_problem_rejects_invalid_limits() {
        let mut problem = valid_problem();
        problem.programs.get_mut("std").unwrap().time_limit_secs = 0.0;

        let err = validate_problem(&problem).unwrap_err().to_string();

        assert!(err.contains("time_limit_secs"));
    }

    #[test]
    fn validate_problem_rejects_missing_generator_references() {
        let mut problem = valid_problem();
        problem.test.bundles.get_mut("sample").unwrap().cases[0].generator_name =
            "missing".to_string();

        let err = validate_problem(&problem).unwrap_err().to_string();

        assert!(err.contains("generator for sample[0] `missing`"));
    }

    #[test]
    fn validate_problem_rejects_missing_bundle_references() {
        let mut problem = valid_problem();
        problem.test.tasks[0].bundles = vec!["missing".to_string()];

        let err = validate_problem(&problem).unwrap_err().to_string();

        assert!(err.contains("references missing bundle `missing`"));
    }

    fn valid_problem() -> Problem {
        let mut programs = HashMap::new();
        programs.insert("gen".to_string(), cpp_program());
        programs.insert("std".to_string(), cpp_program());
        Problem {
            name: "sample".to_string(),
            programs,
            test: Test {
                bundles: HashMap::from([(
                    "sample".to_string(),
                    TestBundle {
                        cases: vec![TestCase {
                            generator_name: "gen".to_string(),
                            args: Vec::new(),
                        }],
                    },
                )]),
                tasks: vec![TestTask {
                    name: "sample".to_string(),
                    score: 100.0,
                    task_type: TestTaskType::Min,
                    bundles: vec!["sample".to_string()],
                    dependencies: Vec::new(),
                }],
            },
            solution_name: "std".to_string(),
            validator_name: None,
            checker_name: None,
        }
    }

    fn cpp_program() -> Program {
        Program {
            info: ProgramInfo::Cpp(CppProgram {
                path: PathBuf::from("main.cpp"),
                compile_args: Vec::new(),
            }),
            time_limit_secs: 1.0,
            memory_limit_mb: 512.0,
        }
    }
}
