use super::problem::{
    case_file_stem, load_problem, normalize_work_dir, parse_case_selector, resolve_path,
};
use super::program::{ProgramSpec, absolutize_program_info, run_spec};
use super::schema::{CaseSelector, Problem};
use super::unix_epoch_nanos;
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const GENERATION_LOCK_DIR: &str = ".cptool-gen.lock";
const GENERATION_STAGING_PREFIX: &str = ".cptool-gen-";

#[derive(Clone, Debug)]
pub struct GenerateOptions {
    pub work_dir: PathBuf,
    pub bundle: Option<String>,
    pub selector: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub output_limit_bytes: usize,
    pub clean: bool,
}

struct GenerateContext<'a> {
    work_dir: &'a Path,
    problem: &'a Problem,
    programs: &'a HashMap<String, ProgramSpec>,
    solution: &'a ProgramSpec,
    validator: Option<&'a ProgramSpec>,
    final_output_dir: &'a Path,
    staging_dir: &'a Path,
    output_limit_bytes: usize,
}

#[derive(Clone, Debug)]
struct GeneratedFile {
    staging_path: PathBuf,
    final_path: PathBuf,
}

struct GeneratedCase {
    files: Vec<GeneratedFile>,
    selector: CaseSelector,
    input_bytes: usize,
    answer_bytes: usize,
    warnings: Vec<GenerateWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GenerateReport {
    pub paths: Vec<PathBuf>,
    pub cases: usize,
    pub bundles: Vec<String>,
    pub elapsed_ms: u128,
    pub input_bytes: usize,
    pub answer_bytes: usize,
    pub warnings: Vec<GenerateWarning>,
}

impl GenerateReport {
    pub fn summary_line(&self) -> String {
        format!(
            "gen: ok cases={} bundles={} elapsed={}ms in_bytes={} ans_bytes={} warnings={}",
            self.cases,
            self.bundles.join(","),
            self.elapsed_ms,
            self.input_bytes,
            self.answer_bytes,
            summarize_generate_warnings(&self.warnings)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GenerateWarning {
    pub kind: GenerateWarningKind,
    pub bundle: String,
    pub case_index: usize,
    pub program: String,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

impl GenerateWarning {
    fn render(&self) -> String {
        match self.kind {
            GenerateWarningKind::GeneratorOutputSuspicious => format!(
                "warning: generator_output_suspicious case={}[{}] generator={} stdout_bytes={} stderr_bytes={}",
                self.bundle, self.case_index, self.program, self.stdout_bytes, self.stderr_bytes
            ),
            GenerateWarningKind::EmptyAnswer => format!(
                "warning: empty_answer case={}[{}] solution={} stdout_bytes={} stderr_bytes={}",
                self.bundle, self.case_index, self.program, self.stdout_bytes, self.stderr_bytes
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerateWarningKind {
    GeneratorOutputSuspicious,
    EmptyAnswer,
}

impl GenerateWarningKind {
    fn as_str(self) -> &'static str {
        match self {
            GenerateWarningKind::GeneratorOutputSuspicious => "generator_output_suspicious",
            GenerateWarningKind::EmptyAnswer => "empty_answer",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataGenerationStatus {
    pub marker_path: PathBuf,
}

#[derive(Debug)]
struct DataGenerationLock {
    path: PathBuf,
}

enum CleanScope {
    Case(CaseSelector),
    Bundle(String),
    All(Vec<String>),
}

pub fn generate_data(
    work_dir: &Path,
    bundle: Option<&str>,
    selector: Option<&str>,
    output_dir: Option<&Path>,
    output_limit_bytes: usize,
) -> Result<Vec<PathBuf>> {
    generate_data_with_options(GenerateOptions {
        work_dir: work_dir.to_path_buf(),
        bundle: bundle.map(str::to_string),
        selector: selector.map(str::to_string),
        output_dir: output_dir.map(Path::to_path_buf),
        output_limit_bytes,
        clean: false,
    })
}

pub fn generate_data_with_options(options: GenerateOptions) -> Result<Vec<PathBuf>> {
    generate_data_report_impl(options, true).map(|report| report.paths)
}

pub fn generate_data_report_with_options(options: GenerateOptions) -> Result<GenerateReport> {
    generate_data_report_impl(options, false)
}

fn generate_data_report_impl(
    options: GenerateOptions,
    emit_warnings: bool,
) -> Result<GenerateReport> {
    let GenerateOptions {
        work_dir,
        bundle,
        selector,
        output_dir,
        output_limit_bytes,
        clean,
    } = options;
    let work_dir = normalize_work_dir(&work_dir)?;
    let problem = load_problem(&work_dir)?;
    let output_dir = output_dir
        .as_deref()
        .map(|path| resolve_path(&work_dir, path))
        .unwrap_or_else(|| work_dir.join("data"));
    std::fs::create_dir_all(&output_dir)?;
    let _generation_lock = DataGenerationLock::acquire(&output_dir)?;
    let selected_cases = select_cases(&problem, bundle.as_deref(), selector.as_deref())?;
    let clean_scope =
        clean.then(|| clean_scope_for(&problem, bundle.as_deref(), selector.as_deref()));
    let staging_dir = create_staging_dir(&output_dir)?;
    let programs = build_program_specs(&work_dir, &problem)?;
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
        final_output_dir: &output_dir,
        staging_dir: &staging_dir,
        output_limit_bytes,
    };

    let start = Instant::now();
    let generated = (|| {
        let mut cases = Vec::new();
        for selector in &selected_cases {
            cases.push(generate_one_case(&context, selector, emit_warnings)?);
        }
        let files = cases
            .iter()
            .flat_map(|case| case.files.iter())
            .cloned()
            .collect::<Vec<_>>();
        let paths = commit_generated_files(&output_dir, clean_scope.as_ref(), &files)?;
        Ok(build_generate_report(
            cases,
            paths,
            start.elapsed().as_millis(),
        ))
    })();
    let cleanup_result = std::fs::remove_dir_all(&staging_dir);
    match (generated, cleanup_result) {
        (Ok(report), Ok(())) => Ok(report),
        (Ok(_), Err(err)) => Err(err).with_context(|| {
            format!(
                "generated data but failed to remove staging dir {}",
                staging_dir.display()
            )
        }),
        (Err(err), _) => Err(err),
    }
}

pub fn data_generation_status(output_dir: &Path) -> Option<DataGenerationStatus> {
    let lock_dir = output_dir.join(GENERATION_LOCK_DIR);
    if lock_dir.is_dir() {
        return Some(DataGenerationStatus {
            marker_path: lock_dir,
        });
    }

    let entries = std::fs::read_dir(output_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(GENERATION_STAGING_PREFIX))
        {
            return Some(DataGenerationStatus { marker_path: path });
        }
    }
    None
}

impl DataGenerationLock {
    fn acquire(output_dir: &Path) -> Result<Self> {
        let path = output_dir.join(GENERATION_LOCK_DIR);
        match std::fs::create_dir(&path) {
            Ok(()) => {
                let metadata = format!(
                    "pid={}\nstarted_nanos={}\n",
                    std::process::id(),
                    unix_epoch_nanos()
                );
                std::fs::write(path.join("owner"), metadata).with_context(|| {
                    format!("failed to write data generation lock {}", path.display())
                })?;
                Ok(Self { path })
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                anyhow::bail!("data generation is already in progress: {}", path.display())
            }
            Err(err) => Err(err).with_context(|| {
                format!("failed to create data generation lock {}", path.display())
            }),
        }
    }
}

impl Drop for DataGenerationLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn build_program_specs(work_dir: &Path, problem: &Problem) -> Result<HashMap<String, ProgramSpec>> {
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
    emit_warnings: bool,
) -> Result<GeneratedCase> {
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
    let case_stem = case_file_stem(selector);
    let staging_stem = context.staging_dir.join(&case_stem);
    let final_stem = context.final_output_dir.join(case_stem);
    let input_path = staging_stem.with_extension("in");
    let answer_path = staging_stem.with_extension("ans");
    let final_input_path = final_stem.with_extension("in");
    let final_answer_path = final_stem.with_extension("ans");
    let generated = run_spec(
        context.work_dir,
        generator,
        &case.args,
        None,
        context.output_limit_bytes,
    )?;
    if !generated.ok {
        anyhow::bail!(
            "{}",
            generated.failure_report(&format!(
                "generator failed for {}[{}]",
                selector.bundle, selector.index
            ))
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
    let mut warnings = Vec::new();
    if generated.stdout_bytes.is_empty() {
        warnings.push(GenerateWarning {
            kind: GenerateWarningKind::GeneratorOutputSuspicious,
            bundle: selector.bundle.clone(),
            case_index: selector.index,
            program: generator.label.clone(),
            stdout_bytes: 0,
            stderr_bytes: generated.stderr_bytes.len(),
        });
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
                "{}",
                validation
                    .failure_report(&format!("validator failed for {}", input_path.display()))
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
            "{}",
            answer.failure_report(&format!("solution failed for {}", input_path.display()))
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
    if !context.problem.output.allow_empty
        && !generated.stdout_bytes.is_empty()
        && answer.stdout_bytes.is_empty()
    {
        warnings.push(GenerateWarning {
            kind: GenerateWarningKind::EmptyAnswer,
            bundle: selector.bundle.clone(),
            case_index: selector.index,
            program: context.solution.label.clone(),
            stdout_bytes: 0,
            stderr_bytes: answer.stderr_bytes.len(),
        });
    }
    std::fs::write(&answer_path, &answer.stdout_bytes)?;
    if emit_warnings {
        for warning in &warnings {
            eprintln!("{}", warning.render());
        }
    }
    Ok(GeneratedCase {
        files: vec![
            GeneratedFile {
                staging_path: input_path,
                final_path: final_input_path,
            },
            GeneratedFile {
                staging_path: answer_path,
                final_path: final_answer_path,
            },
        ],
        selector: selector.clone(),
        input_bytes: generated.stdout_bytes.len(),
        answer_bytes: answer.stdout_bytes.len(),
        warnings,
    })
}

fn build_generate_report(
    cases: Vec<GeneratedCase>,
    paths: Vec<PathBuf>,
    elapsed_ms: u128,
) -> GenerateReport {
    let mut bundles = BTreeSet::new();
    let mut input_bytes = 0;
    let mut answer_bytes = 0;
    let mut warnings = Vec::new();
    let case_count = cases.len();

    for case in cases {
        bundles.insert(case.selector.bundle);
        input_bytes += case.input_bytes;
        answer_bytes += case.answer_bytes;
        warnings.extend(case.warnings);
    }

    GenerateReport {
        paths,
        cases: case_count,
        bundles: bundles.into_iter().collect(),
        elapsed_ms,
        input_bytes,
        answer_bytes,
        warnings,
    }
}

fn summarize_generate_warnings(warnings: &[GenerateWarning]) -> String {
    if warnings.is_empty() {
        return "0".to_string();
    }

    let mut counts = BTreeMap::new();
    for warning in warnings {
        *counts.entry(warning.kind.as_str()).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .map(|(kind, count)| format!("{kind}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn select_cases(
    problem: &Problem,
    bundle: Option<&str>,
    selector: Option<&str>,
) -> Result<Vec<CaseSelector>> {
    if let Some(selector) = selector {
        let selector = parse_case_selector(selector)?;
        ensure_case_exists(problem, &selector)?;
        return Ok(vec![selector]);
    }
    if let Some(bundle) = bundle {
        let bundle_cases = problem
            .test
            .bundles
            .get(bundle)
            .with_context(|| format!("bundle `{bundle}` not found"))?;
        return Ok((0..bundle_cases.cases.len())
            .map(|index| CaseSelector {
                bundle: bundle.to_string(),
                index,
            })
            .collect());
    }

    let mut selectors = Vec::new();
    for (bundle, bundle_cases) in &problem.test.bundles {
        for index in 0..bundle_cases.cases.len() {
            selectors.push(CaseSelector {
                bundle: bundle.clone(),
                index,
            });
        }
    }
    Ok(selectors)
}

fn ensure_case_exists(problem: &Problem, selector: &CaseSelector) -> Result<()> {
    let bundle = problem
        .test
        .bundles
        .get(&selector.bundle)
        .with_context(|| format!("bundle `{}` not found", selector.bundle))?;
    bundle
        .cases
        .get(selector.index)
        .with_context(|| format!("case `{}` not found", selector.index))?;
    Ok(())
}

fn clean_scope_for(problem: &Problem, bundle: Option<&str>, selector: Option<&str>) -> CleanScope {
    if let Some(selector) = selector {
        return CleanScope::Case(
            parse_case_selector(selector)
                .expect("selector was already parsed successfully before clean scope creation"),
        );
    }
    if let Some(bundle) = bundle {
        return CleanScope::Bundle(bundle.to_string());
    }
    CleanScope::All(problem.test.bundles.keys().cloned().collect())
}

fn create_staging_dir(output_dir: &Path) -> Result<PathBuf> {
    for attempt in 0..100 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before UNIX_EPOCH")?
            .as_nanos();
        let dir = output_dir.join(format!(
            "{GENERATION_STAGING_PREFIX}{}-{nanos}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to create staging dir {}", dir.display()));
            }
        }
    }
    anyhow::bail!("failed to allocate a unique staging directory");
}

fn commit_generated_files(
    output_dir: &Path,
    clean_scope: Option<&CleanScope>,
    generated: &[GeneratedFile],
) -> Result<Vec<PathBuf>> {
    for file in generated {
        if !file.staging_path.is_file() {
            anyhow::bail!(
                "staged generated file is missing: {}",
                file.staging_path.display()
            );
        }
    }

    if let Some(scope) = clean_scope {
        for path in clean_paths(output_dir, scope)? {
            remove_file_if_exists(&path)?;
        }
    }

    let mut final_paths = Vec::with_capacity(generated.len());
    for file in generated {
        remove_file_if_exists(&file.final_path)?;
        std::fs::rename(&file.staging_path, &file.final_path).with_context(|| {
            format!(
                "failed to move {} to {}",
                file.staging_path.display(),
                file.final_path.display()
            )
        })?;
        final_paths.push(file.final_path.clone());
    }
    Ok(final_paths)
}

fn clean_paths(output_dir: &Path, scope: &CleanScope) -> Result<Vec<PathBuf>> {
    match scope {
        CleanScope::Case(selector) => {
            let stem = output_dir.join(case_file_stem(selector));
            Ok(vec![stem.with_extension("in"), stem.with_extension("ans")])
        }
        CleanScope::Bundle(bundle) => matching_bundle_files(output_dir, &[bundle.as_str()]),
        CleanScope::All(bundles) => matching_bundle_files(
            output_dir,
            &bundles.iter().map(String::as_str).collect::<Vec<_>>(),
        ),
    }
}

fn matching_bundle_files(output_dir: &Path, bundles: &[&str]) -> Result<Vec<PathBuf>> {
    if !output_dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(output_dir)
        .with_context(|| format!("failed to read output dir {}", output_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !is_data_file(&path) {
            continue;
        }
        let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if bundles
            .iter()
            .any(|bundle| file_stem.starts_with(&format!("{bundle}-")))
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn is_data_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("in" | "ans")
    )
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_bundle_only_targets_matching_bundle_data_files() {
        let root = temp_dir("clean-bundle");
        std::fs::create_dir_all(&root).unwrap();
        write_file(&root.join("large-0.in"), "old");
        write_file(&root.join("large-99.ans"), "stale");
        write_file(&root.join("small-0.in"), "keep");
        write_file(&root.join("largeish-0.in"), "keep");
        write_file(&root.join("large-0.txt"), "keep");

        let mut names = clean_paths(&root, &CleanScope::Bundle("large".to_string()))
            .unwrap()
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();

        assert_eq!(names, vec!["large-0.in", "large-99.ans"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn commit_preflight_preserves_existing_files_when_staging_is_incomplete() {
        let root = temp_dir("commit-preflight");
        let staging = root.join(".stage");
        std::fs::create_dir_all(&staging).unwrap();
        let final_input = root.join("sample-0.in");
        write_file(&final_input, "previous");

        let err = commit_generated_files(
            &root,
            None,
            &[GeneratedFile {
                staging_path: staging.join("sample-0.in"),
                final_path: final_input.clone(),
            }],
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("staged generated file is missing"));
        assert_eq!(std::fs::read_to_string(&final_input).unwrap(), "previous");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn commit_clean_bundle_does_not_remove_other_bundles() {
        let root = temp_dir("commit-clean-bundle");
        let staging = root.join(".stage");
        std::fs::create_dir_all(&staging).unwrap();
        write_file(&root.join("large-9.in"), "stale");
        write_file(&root.join("small-0.in"), "keep");
        let staged_input = staging.join("large-0.in");
        write_file(&staged_input, "fresh");

        let paths = commit_generated_files(
            &root,
            Some(&CleanScope::Bundle("large".to_string())),
            &[GeneratedFile {
                staging_path: staged_input,
                final_path: root.join("large-0.in"),
            }],
        )
        .unwrap();

        assert_eq!(paths, vec![root.join("large-0.in")]);
        assert_eq!(
            std::fs::read_to_string(root.join("large-0.in")).unwrap(),
            "fresh"
        );
        assert!(!root.join("large-9.in").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("small-0.in")).unwrap(),
            "keep"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generation_lock_marks_output_dir_and_blocks_second_writer() {
        let root = temp_dir("generation-lock");
        std::fs::create_dir_all(&root).unwrap();

        let lock = DataGenerationLock::acquire(&root).unwrap();
        let status = data_generation_status(&root).unwrap();

        assert_eq!(status.marker_path, root.join(GENERATION_LOCK_DIR));
        assert!(
            DataGenerationLock::acquire(&root)
                .unwrap_err()
                .to_string()
                .contains("data generation is already in progress")
        );

        drop(lock);
        assert!(data_generation_status(&root).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generation_status_detects_existing_staging_dir() {
        let root = temp_dir("generation-staging");
        let staging = root.join(format!("{GENERATION_STAGING_PREFIX}123-456-0"));
        std::fs::create_dir_all(&staging).unwrap();

        assert_eq!(data_generation_status(&root).unwrap().marker_path, staging);

        std::fs::remove_dir_all(root).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        super::super::temp_test_dir(&format!("cptool-data-{label}"))
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
}
