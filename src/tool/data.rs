use super::judge::fix_validator_input_line_endings;
use super::problem::{
    FILE_GENERATOR_NAME, case_file_stem, load_problem, normalize_work_dir, parse_case_selector,
    resolve_path,
};
use super::program::{ProgramSpec, absolutize_program_info, run_spec};
use super::schema::{CaseSelector, Problem};
use super::unix_epoch_nanos;
use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GENERATION_LOCK_DIR: &str = ".cptool-gen.lock";
const GENERATION_STAGING_PREFIX: &str = ".cptool-gen-";
const GENERATION_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
pub struct GenerateOptions {
    pub work_dir: PathBuf,
    pub bundle: Option<String>,
    pub selector: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub output_limit_bytes: usize,
    pub generation_lock_timeout: Option<Duration>,
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

struct StagingDirGuard {
    path: Option<PathBuf>,
}

impl StagingDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("staging dir guard path should be present")
    }

    fn cleanup(mut self) -> Result<()> {
        let path = self
            .path
            .take()
            .expect("staging dir guard path should be present");
        std::fs::remove_dir_all(&path).with_context(|| {
            format!(
                "generated data but failed to remove staging dir {}",
                path.display()
            )
        })
    }
}

impl Drop for StagingDirGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

#[derive(Clone, Debug)]
struct GeneratedFile {
    staging_path: PathBuf,
    final_path: PathBuf,
}

struct GeneratedCase {
    files: Vec<GeneratedFile>,
    selector: CaseSelector,
    input_hash: Vec<u8>,
    input_bytes: usize,
    answer_bytes: usize,
    validator_calls: usize,
    warnings: Vec<GenerateWarning>,
}

pub(crate) struct GeneratorInput {
    pub(crate) label: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) stderr_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GenerateReport {
    pub paths: Vec<PathBuf>,
    pub cases: usize,
    pub bundles: Vec<String>,
    pub elapsed_ms: u128,
    pub input_bytes: usize,
    pub answer_bytes: usize,
    pub validator_configured: bool,
    pub validator_calls: usize,
    pub warnings: Vec<GenerateWarning>,
}

impl GenerateReport {
    pub fn summary_line(&self) -> String {
        format!(
            "gen: ok cases={} bundles={} elapsed={}ms in_bytes={} ans_bytes={} validator_calls={} warnings={}",
            self.cases,
            self.bundles.join(","),
            self.elapsed_ms,
            self.input_bytes,
            self.answer_bytes,
            self.validator_calls,
            summarize_generate_warnings(&self.warnings)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GenerateWarning {
    pub kind: GenerateWarningKind,
    pub bundle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_index: Option<usize>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub program: String,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_input_hashes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub random_coverage: Option<bool>,
}

impl GenerateWarning {
    fn render(&self) -> String {
        match self.kind {
            GenerateWarningKind::GeneratorOutputSuspicious => format!(
                "warning: generator_output_suspicious case={}[{}] generator={} stdout_bytes={} stderr_bytes={}",
                self.bundle,
                self.case_index.unwrap_or(0),
                self.program,
                self.stdout_bytes,
                self.stderr_bytes
            ),
            GenerateWarningKind::EmptyAnswer => format!(
                "warning: empty_answer case={}[{}] solution={} stdout_bytes={} stderr_bytes={}",
                self.bundle,
                self.case_index.unwrap_or(0),
                self.program,
                self.stdout_bytes,
                self.stderr_bytes
            ),
            GenerateWarningKind::RepeatedInput => format!(
                "warning: repeated_input bundle={} cases={} unique_input_hashes={} random_coverage=false hint=all_inputs_identical_within_bundle",
                self.bundle,
                self.case_count.unwrap_or(0),
                self.unique_input_hashes.unwrap_or(1)
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerateWarningKind {
    GeneratorOutputSuspicious,
    EmptyAnswer,
    RepeatedInput,
}

impl GenerateWarningKind {
    fn as_str(self) -> &'static str {
        match self {
            GenerateWarningKind::GeneratorOutputSuspicious => "generator_output_suspicious",
            GenerateWarningKind::EmptyAnswer => "empty_answer",
            GenerateWarningKind::RepeatedInput => "repeated_input",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DataGenerationStatus {
    pub marker_path: PathBuf,
}

#[derive(Debug)]
struct DataGenerationLock {
    path: PathBuf,
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
        generation_lock_timeout,
    } = options;
    let work_dir = normalize_work_dir(&work_dir)?;
    let problem = load_problem(&work_dir)?;
    let output_dir = output_dir
        .as_deref()
        .map(|path| resolve_path(&work_dir, path))
        .unwrap_or_else(|| work_dir.join("data"));
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create output dir {}", output_dir.display()))?;
    let _generation_lock = DataGenerationLock::acquire(&output_dir, generation_lock_timeout)?;
    let selected_cases = select_cases(&problem, bundle.as_deref(), selector.as_deref())?;
    let staging_dir = StagingDirGuard::new(create_staging_dir(&output_dir)?);
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
        staging_dir: staging_dir.path(),
        output_limit_bytes,
    };

    let start = Instant::now();
    let generated = (|| {
        let mut cases = Vec::new();
        for selector in &selected_cases {
            cases.push(generate_one_case(&context, selector)?);
        }
        let files = cases
            .iter()
            .flat_map(|case| case.files.iter())
            .cloned()
            .collect::<Vec<_>>();
        let paths = commit_generated_files(&output_dir, staging_dir.path(), &files)?;
        let report = build_generate_report(
            cases,
            paths,
            start.elapsed().as_millis(),
            context.validator.is_some(),
        );
        if emit_warnings {
            for warning in &report.warnings {
                eprintln!("{}", warning.render());
            }
        }
        Ok(report)
    })();
    match generated {
        Ok(report) => {
            staging_dir.cleanup()?;
            Ok(report)
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn data_generation_status(output_dir: &Path) -> Option<DataGenerationStatus> {
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

pub(crate) fn wait_for_generation_status(
    output_dir: &Path,
    timeout: Duration,
) -> Option<DataGenerationStatus> {
    let initial = data_generation_status(output_dir)?;
    eprintln!(
        "waiting for data generation lock: {} timeout={}",
        initial.marker_path.display(),
        format_duration(timeout)
    );

    let start = Instant::now();
    loop {
        if let Some(status) = data_generation_status(output_dir) {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Some(status);
            }
            let remaining = timeout.saturating_sub(elapsed);
            std::thread::sleep(remaining.min(GENERATION_LOCK_POLL_INTERVAL));
            continue;
        }
        return None;
    }
}

impl DataGenerationLock {
    fn acquire(output_dir: &Path, timeout: Option<Duration>) -> Result<Self> {
        if let Some(timeout) = timeout
            && let Some(status) = wait_for_generation_status(output_dir, timeout)
        {
            anyhow::bail!(
                "data generation is already in progress: {} (waited {}; retry after current generation finishes or prewarm the selector serially)",
                status.marker_path.display(),
                format_duration(timeout)
            );
        }
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
                anyhow::bail!(
                    "data generation is already in progress: {} (retry after current generation finishes or prewarm the selector serially)",
                    path.display()
                )
            }
            Err(err) => Err(err).with_context(|| {
                format!("failed to create data generation lock {}", path.display())
            }),
        }
    }
}

pub(crate) fn format_duration(duration: Duration) -> String {
    if duration.subsec_nanos() == 0 {
        format!("{}s", duration.as_secs())
    } else {
        format!("{:.3}s", duration.as_secs_f64())
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
    let case_stem = case_file_stem(selector);
    let staging_stem = context.staging_dir.join(&case_stem);
    let final_stem = context.final_output_dir.join(case_stem);
    let input_path = staging_stem.with_extension("in");
    let answer_path = staging_stem.with_extension("ans");
    let final_input_path = final_stem.with_extension("in");
    let final_answer_path = final_stem.with_extension("ans");
    let generated = generate_case_input(
        context.work_dir,
        context.programs,
        &case.generator_name,
        &case.args,
        context.output_limit_bytes,
        &format!("{}[{}]", selector.bundle, selector.index),
    )?;
    let mut warnings = Vec::new();
    if generated.bytes.is_empty() {
        warnings.push(GenerateWarning {
            kind: GenerateWarningKind::GeneratorOutputSuspicious,
            bundle: selector.bundle.clone(),
            case_index: Some(selector.index),
            program: generated.label.clone(),
            stdout_bytes: 0,
            stderr_bytes: generated.stderr_bytes,
            case_count: None,
            unique_input_hashes: None,
            random_coverage: None,
        });
    }
    std::fs::write(&input_path, &generated.bytes).with_context(|| {
        format!(
            "failed to write generated input for {}[{}] to {}",
            selector.bundle,
            selector.index,
            input_path.display()
        )
    })?;
    let mut validator_calls = 0;
    if let Some(validator) = context.validator {
        validator_calls += 1;
        let validation = run_spec(
            context.work_dir,
            validator,
            &[],
            Some(&generated.bytes),
            context.output_limit_bytes,
        )?;
        if !validation.is_success() {
            anyhow::bail!(
                "{}",
                validation.failure_report(&format!(
                    "validator failed for {}[{}] input={} generator={} args={:?}",
                    selector.bundle,
                    selector.index,
                    input_path.display(),
                    case.generator_name,
                    case.args
                ))
            );
        }
    }
    let answer = run_spec(
        context.work_dir,
        context.solution,
        &[],
        Some(&generated.bytes),
        context.output_limit_bytes,
    )?;
    if !answer.is_success() {
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
        && !generated.bytes.is_empty()
        && answer.stdout_bytes.is_empty()
    {
        warnings.push(GenerateWarning {
            kind: GenerateWarningKind::EmptyAnswer,
            bundle: selector.bundle.clone(),
            case_index: Some(selector.index),
            program: context.solution.label.clone(),
            stdout_bytes: 0,
            stderr_bytes: answer.stderr_bytes.len(),
            case_count: None,
            unique_input_hashes: None,
            random_coverage: None,
        });
    }
    std::fs::write(&answer_path, &answer.stdout_bytes).with_context(|| {
        format!(
            "failed to write generated answer for {}[{}] to {}",
            selector.bundle,
            selector.index,
            answer_path.display()
        )
    })?;
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
        input_hash: Sha256::digest(&generated.bytes).to_vec(),
        input_bytes: generated.bytes.len(),
        answer_bytes: answer.stdout_bytes.len(),
        validator_calls,
        warnings,
    })
}

pub(crate) fn generate_case_input(
    work_dir: &Path,
    programs: &HashMap<String, ProgramSpec>,
    generator_name: &str,
    args: &[String],
    output_limit_bytes: usize,
    context: &str,
) -> Result<GeneratorInput> {
    if generator_name == FILE_GENERATOR_NAME {
        return read_file_generator_input(work_dir, args, context);
    }
    if generator_name.starts_with(':') {
        anyhow::bail!("generator `{generator_name}` is an unknown built-in generator");
    }
    let generator = programs
        .get(generator_name)
        .with_context(|| format!("generator `{generator_name}` not found"))?;
    let generated = run_spec(work_dir, generator, args, None, output_limit_bytes)?;
    if !generated.is_success() {
        anyhow::bail!(
            "{}",
            generated.failure_report(&format!("generator failed for {context}"))
        );
    }
    if generated.truncated_stdout {
        anyhow::bail!(
            "generator output for {context} exceeded --output-limit-bytes ({output_limit_bytes})"
        );
    }
    Ok(GeneratorInput {
        label: generator.label.clone(),
        bytes: generated.stdout_bytes,
        stderr_bytes: generated.stderr_bytes.len(),
    })
}

pub(crate) fn read_file_generator_input(
    work_dir: &Path,
    args: &[String],
    context: &str,
) -> Result<GeneratorInput> {
    if args.len() != 1 {
        anyhow::bail!(
            "generator `{FILE_GENERATOR_NAME}` for {context} expects exactly one input path argument, got {}",
            args.len()
        );
    }
    let source_path = resolve_path(work_dir, Path::new(&args[0]));
    ensure_file_generator_fixture_path(work_dir, &source_path, &args[0], context)?;
    let mut bytes = std::fs::read(&source_path).with_context(|| {
        format!(
            "failed to read `{FILE_GENERATOR_NAME}` input for {context}: {}",
            source_path.display()
        )
    })?;
    let _ = fix_validator_input_line_endings(&mut bytes);
    Ok(GeneratorInput {
        label: FILE_GENERATOR_NAME.to_string(),
        bytes,
        stderr_bytes: 0,
    })
}

fn ensure_file_generator_fixture_path(
    work_dir: &Path,
    source_path: &Path,
    raw_arg: &str,
    context: &str,
) -> Result<()> {
    let fixture_dir = normalize_path_lexically(&work_dir.join("fixtures").join("input"));
    let source_path = normalize_path_lexically(source_path);
    if !source_path.starts_with(&fixture_dir) {
        anyhow::bail!(
            "generator `{FILE_GENERATOR_NAME}` for {context} must read handwritten input from fixtures/input; got `{raw_arg}`"
        );
    }
    if source_path.extension().and_then(|ext| ext.to_str()) != Some("in") {
        anyhow::bail!(
            "generator `{FILE_GENERATOR_NAME}` for {context} must read a .in fixture under fixtures/input; got `{raw_arg}`"
        );
    }
    if let (Ok(source), Ok(fixture_dir)) = (
        std::fs::canonicalize(&source_path),
        std::fs::canonicalize(&fixture_dir),
    ) && !source.starts_with(&fixture_dir)
    {
        anyhow::bail!(
            "generator `{FILE_GENERATOR_NAME}` for {context} must read handwritten input from fixtures/input; got `{raw_arg}`"
        );
    }
    Ok(())
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn build_generate_report(
    cases: Vec<GeneratedCase>,
    paths: Vec<PathBuf>,
    elapsed_ms: u128,
    validator_configured: bool,
) -> GenerateReport {
    let mut bundles = BTreeSet::new();
    let mut input_bytes = 0;
    let mut answer_bytes = 0;
    let mut validator_calls = 0;
    let mut warnings = Vec::new();
    let mut bundle_input_hashes = BTreeMap::<String, Vec<Vec<u8>>>::new();
    let case_count = cases.len();

    for case in cases {
        bundle_input_hashes
            .entry(case.selector.bundle.clone())
            .or_default()
            .push(case.input_hash);
        bundles.insert(case.selector.bundle.clone());
        input_bytes += case.input_bytes;
        answer_bytes += case.answer_bytes;
        validator_calls += case.validator_calls;
        warnings.extend(case.warnings);
    }
    for (bundle, hashes) in bundle_input_hashes {
        let unique = hashes.iter().collect::<HashSet<_>>().len();
        if hashes.len() > 1 && unique == 1 {
            warnings.push(GenerateWarning {
                kind: GenerateWarningKind::RepeatedInput,
                bundle,
                case_index: None,
                program: String::new(),
                stdout_bytes: 0,
                stderr_bytes: 0,
                case_count: Some(hashes.len()),
                unique_input_hashes: Some(unique),
                random_coverage: Some(false),
            });
        }
    }

    GenerateReport {
        paths,
        cases: case_count,
        bundles: bundles.into_iter().collect(),
        elapsed_ms,
        input_bytes,
        answer_bytes,
        validator_configured,
        validator_calls,
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
    let official = official_bundle_names(problem);
    for (bundle, bundle_cases) in &problem.test.bundles {
        if !official.contains(bundle) {
            continue;
        }
        for index in 0..bundle_cases.cases.len() {
            selectors.push(CaseSelector {
                bundle: bundle.clone(),
                index,
            });
        }
    }
    Ok(selectors)
}

pub(crate) fn official_bundle_names(problem: &Problem) -> HashSet<String> {
    problem
        .test
        .tasks
        .iter()
        .filter(|task| task.is_official())
        .flat_map(|task| task.bundles.iter().cloned())
        .collect()
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
    staging_dir: &Path,
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

    replace_output_dir_contents(output_dir, staging_dir)?;

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

fn replace_output_dir_contents(output_dir: &Path, staging_dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(output_dir)
        .with_context(|| format!("failed to read output dir {}", output_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path == staging_dir
            || path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == GENERATION_LOCK_DIR)
        {
            continue;
        }
        if path.is_dir() {
            std::fs::remove_dir_all(&path).with_context(|| {
                format!("failed to remove stale data directory {}", path.display())
            })?;
        } else {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale data file {}", path.display()))?;
        }
    }
    Ok(())
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
    fn commit_replaces_output_dir_contents() {
        let root = temp_dir("replace-output");
        let staging = root.join(".stage");
        let lock = root.join(GENERATION_LOCK_DIR);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::create_dir_all(&lock).unwrap();
        write_file(&root.join("sample-0.in"), "old");
        write_file(&root.join("notes.txt"), "manual");
        write_file(&root.join("nested").join("manual.in"), "manual");
        let staged_input = staging.join("sample-0.in");
        write_file(&staged_input, "fresh");

        let paths = commit_generated_files(
            &root,
            &staging,
            &[GeneratedFile {
                staging_path: staged_input,
                final_path: root.join("sample-0.in"),
            }],
        )
        .unwrap();

        assert_eq!(paths, vec![root.join("sample-0.in")]);
        assert_eq!(
            std::fs::read_to_string(root.join("sample-0.in")).unwrap(),
            "fresh"
        );
        assert!(!root.join("notes.txt").exists());
        assert!(!root.join("nested").exists());
        assert!(lock.is_dir());
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
            &staging,
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
    fn commit_replaces_everything_even_for_single_generated_file() {
        let root = temp_dir("commit-replace-all");
        let staging = root.join(".stage");
        std::fs::create_dir_all(&staging).unwrap();
        write_file(&root.join("large-9.in"), "stale");
        write_file(&root.join("small-0.in"), "stale");
        let staged_input = staging.join("large-0.in");
        write_file(&staged_input, "fresh");

        let paths = commit_generated_files(
            &root,
            &staging,
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
        assert!(!root.join("small-0.in").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generation_lock_marks_output_dir_and_blocks_second_writer() {
        let root = temp_dir("generation-lock");
        std::fs::create_dir_all(&root).unwrap();

        let lock = DataGenerationLock::acquire(&root, None).unwrap();
        let status = data_generation_status(&root).unwrap();

        assert_eq!(status.marker_path, root.join(GENERATION_LOCK_DIR));
        assert!(
            DataGenerationLock::acquire(&root, None)
                .unwrap_err()
                .to_string()
                .contains("data generation is already in progress")
        );

        drop(lock);
        assert!(data_generation_status(&root).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generation_lock_waits_until_existing_lock_is_released() {
        let root = temp_dir("generation-lock-wait");
        std::fs::create_dir_all(&root).unwrap();
        let lock_dir = root.join(GENERATION_LOCK_DIR);
        std::fs::create_dir_all(&lock_dir).unwrap();

        let lock_dir_for_thread = lock_dir.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(25));
            std::fs::remove_dir_all(lock_dir_for_thread).unwrap();
        });

        let lock = DataGenerationLock::acquire(&root, Some(Duration::from_secs(3))).unwrap();

        handle.join().unwrap();
        assert_eq!(lock.path, lock_dir);
        drop(lock);
        assert!(data_generation_status(&root).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generation_lock_wait_reports_timeout() {
        let root = temp_dir("generation-lock-timeout");
        std::fs::create_dir_all(root.join(GENERATION_LOCK_DIR)).unwrap();

        let err = DataGenerationLock::acquire(&root, Some(Duration::from_millis(1)))
            .unwrap_err()
            .to_string();

        assert!(err.contains("data generation is already in progress"));
        assert!(err.contains("waited 0.001s"));
        assert!(err.contains("prewarm the selector serially"));
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
