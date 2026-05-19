use crate::config::{problem as config_problem, program as config_program};
use anyhow::{Context, Result};
use process_control::{ChildExt, Control};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

const DEFAULT_TIME_LIMIT_SECS: f64 = 1.0;
const DEFAULT_MEMORY_LIMIT_MB: f64 = 512.0;
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 33_554_432;

#[derive(Clone, Debug)]
pub struct CaseSelector {
    pub bundle: String,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub work_dir: PathBuf,
    pub program: Option<String>,
    pub source: Option<PathBuf>,
    pub selector: Option<String>,
    pub stdin_text: Option<String>,
    pub stdin_path: Option<PathBuf>,
    pub stdout_path: Option<PathBuf>,
    pub stderr_path: Option<PathBuf>,
    pub args: Vec<String>,
    pub output_limit_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub label: String,
    pub ok: bool,
    pub kind: String,
    pub exit_code: Option<i32>,
    pub elapsed_ms: u128,
    pub stdout_bytes: Vec<u8>,
    pub stderr_bytes: Vec<u8>,
    pub stdout: String,
    pub stderr: String,
    pub truncated_stdout: bool,
    pub truncated_stderr: bool,
}

#[derive(Clone, Debug)]
struct ProgramSpec {
    label: String,
    info: config_program::ProgramInfo,
    time_limit_secs: f64,
    memory_limit_mb: f64,
}

pub fn init_package(root: &Path, id: &str) -> Result<PathBuf> {
    let slug = slugify(id)?;
    let problem_dir = root.join("problems").join(slug);
    if problem_dir.exists() {
        anyhow::bail!("problem package already exists: {}", problem_dir.display());
    }

    std::fs::create_dir_all(problem_dir.join("src"))?;
    std::fs::create_dir_all(problem_dir.join("data"))?;
    std::fs::create_dir_all(problem_dir.join("tests").join("failures"))?;
    std::fs::write(problem_dir.join("statement.md"), "# 题面\n\n")?;
    std::fs::write(problem_dir.join("editorial.md"), "# 题解\n\n")?;
    std::fs::write(
        problem_dir.join(".gitignore"),
        ".cptool/\ndata/\nexport/\noutput/\ntmp/\ntests/failures/\n*.exe\n*.tmp\n",
    )?;
    std::fs::write(problem_dir.join("src").join("std.cpp"), "")?;
    std::fs::write(problem_dir.join("src").join("brute.cpp"), "")?;
    std::fs::write(problem_dir.join("src").join("gen.cpp"), "")?;
    std::fs::write(
        problem_dir.join("problem.yaml"),
        format!(
            "name: {id}\nprograms:\n  gen:\n    info: !cpp\n      path: ./src/gen.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\n  std:\n    info: !cpp\n      path: ./src/std.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\n  brute:\n    info: !cpp\n      path: ./src/brute.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\nsolution: std\ntest:\n  bundles:\n    sample:\n      cases:\n      - generator: gen\n        args: []\n  tasks:\n  - name: sample\n    score: 100.0\n    type: min\n    bundles: [sample]\n",
        ),
    )?;
    Ok(problem_dir)
}

pub fn slugify(value: &str) -> Result<String> {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch == '-' || !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        anyhow::bail!("problem id cannot be empty");
    }
    Ok(slug)
}

pub fn load_problem(work_dir: &Path) -> Result<config_problem::Problem> {
    let path = work_dir.join("problem.yaml");
    let yaml = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&yaml).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn parse_case_selector(value: &str) -> Result<CaseSelector> {
    let Some(open) = value.rfind('[') else {
        anyhow::bail!(
            "case selector must look like bundle[index], got `{}`",
            value
        );
    };
    if !value.ends_with(']') {
        anyhow::bail!(
            "case selector must look like bundle[index], got `{}`",
            value
        );
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

    let mut generated = Vec::new();
    if let Some(selector) = selector {
        let selector = parse_case_selector(selector)?;
        generated.extend(generate_one_case(
            &work_dir,
            &problem,
            &programs,
            solution,
            validator,
            &output_dir,
            &selector,
            output_limit_bytes,
        )?);
    } else if let Some(bundle) = bundle {
        let bundle_cases = problem
            .test
            .bundles
            .get(bundle)
            .with_context(|| format!("bundle `{bundle}` not found"))?;
        for index in 0..bundle_cases.cases.len() {
            generated.extend(generate_one_case(
                &work_dir,
                &problem,
                &programs,
                solution,
                validator,
                &output_dir,
                &CaseSelector {
                    bundle: bundle.to_string(),
                    index,
                },
                output_limit_bytes,
            )?);
        }
    } else {
        for (bundle, bundle_cases) in &problem.test.bundles {
            for index in 0..bundle_cases.cases.len() {
                generated.extend(generate_one_case(
                    &work_dir,
                    &problem,
                    &programs,
                    solution,
                    validator,
                    &output_dir,
                    &CaseSelector {
                        bundle: bundle.clone(),
                        index,
                    },
                    output_limit_bytes,
                )?);
            }
        }
    }
    Ok(generated)
}

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

fn resolve_run_spec(
    work_dir: &Path,
    problem: &config_problem::Problem,
    program: Option<&str>,
    source: Option<&Path>,
) -> Result<ProgramSpec> {
    if let Some(source) = source {
        return spec_from_source("source", source);
    }
    let name = program.unwrap_or(&problem.solution_name);
    let program = problem
        .programs
        .get(name)
        .with_context(|| format!("program `{name}` not found in problem.yaml"))?;
    Ok(ProgramSpec {
        label: name.to_string(),
        info: absolutize_program_info(work_dir, &program.info),
        time_limit_secs: program.time_limit_secs,
        memory_limit_mb: program.memory_limit_mb,
    })
}

fn resolve_named_or_source(
    work_dir: &Path,
    problem: &config_problem::Problem,
    value: &str,
) -> Result<ProgramSpec> {
    if let Some(program) = problem.programs.get(value) {
        return Ok(ProgramSpec {
            label: value.to_string(),
            info: absolutize_program_info(work_dir, &program.info),
            time_limit_secs: program.time_limit_secs,
            memory_limit_mb: program.memory_limit_mb,
        });
    }
    spec_from_source(value, Path::new(value))
}

fn spec_from_source(label: &str, source: &Path) -> Result<ProgramSpec> {
    let info = match source.extension().and_then(|ext| ext.to_str()) {
        Some("cpp") | Some("cc") | Some("cxx") => {
            config_program::ProgramInfo::Cpp(config_program::CppProgram {
                path: source.to_path_buf(),
                compile_args: vec![
                    "-O2".to_string(),
                    "-std=c++20".to_string(),
                    "-Wall".to_string(),
                    "-Wextra".to_string(),
                    "-pedantic".to_string(),
                ],
            })
        }
        Some("py") => config_program::ProgramInfo::Python(config_program::CommandProgram {
            path: source.to_path_buf(),
            extra_args: vec![],
        }),
        _ => anyhow::bail!(
            "cannot infer program type from source `{}`",
            source.display()
        ),
    };
    Ok(ProgramSpec {
        label: label.to_string(),
        info,
        time_limit_secs: DEFAULT_TIME_LIMIT_SECS,
        memory_limit_mb: DEFAULT_MEMORY_LIMIT_MB,
    })
}

fn resolve_run_input(
    work_dir: &Path,
    problem: &config_problem::Problem,
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

fn default_selector(problem: &config_problem::Problem) -> Result<CaseSelector> {
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

fn compile_programs(
    work_dir: &Path,
    problem: &config_problem::Problem,
) -> Result<HashMap<String, ProgramSpec>> {
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
    work_dir: &Path,
    problem: &config_problem::Problem,
    programs: &HashMap<String, ProgramSpec>,
    solution: &ProgramSpec,
    validator: Option<&ProgramSpec>,
    output_dir: &Path,
    selector: &CaseSelector,
    output_limit_bytes: usize,
) -> Result<Vec<PathBuf>> {
    let bundle = problem
        .test
        .bundles
        .get(&selector.bundle)
        .with_context(|| format!("bundle `{}` not found", selector.bundle))?;
    let case = bundle
        .cases
        .get(selector.index)
        .with_context(|| format!("case `{}` not found", selector.index))?;
    let generator = programs
        .get(&case.generator_name)
        .with_context(|| format!("generator `{}` not found", case.generator_name))?;
    let stem = output_dir.join(case_file_stem(selector));
    let input_path = stem.with_extension("in");
    let answer_path = stem.with_extension("ans");
    let generated = run_spec(work_dir, generator, &case.args, None, output_limit_bytes)?;
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
            "generator output for {}[{}] exceeded --output-limit-bytes ({output_limit_bytes})",
            selector.bundle,
            selector.index
        );
    }
    std::fs::write(&input_path, &generated.stdout_bytes)?;
    if let Some(validator) = validator {
        let validation = run_spec(
            work_dir,
            validator,
            &[],
            Some(&generated.stdout_bytes),
            output_limit_bytes,
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
        work_dir,
        solution,
        &[],
        Some(&generated.stdout_bytes),
        output_limit_bytes,
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
            "solution output for {}[{}] exceeded --output-limit-bytes ({output_limit_bytes})",
            selector.bundle,
            selector.index
        );
    }
    std::fs::write(&answer_path, &answer.stdout_bytes)?;
    Ok(vec![input_path, answer_path])
}

fn run_spec(
    work_dir: &Path,
    spec: &ProgramSpec,
    args: &[String],
    input: Option<&[u8]>,
    output_limit_bytes: usize,
) -> Result<RunResult> {
    let mut command = match &spec.info {
        config_program::ProgramInfo::Command(program) => {
            let mut command = std::process::Command::new(&program.path);
            command.args(&program.extra_args);
            command
        }
        config_program::ProgramInfo::Python(program) => {
            let mut command = std::process::Command::new(
                std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string()),
            );
            command
                .arg("-I")
                .arg(&program.path)
                .args(&program.extra_args);
            command
        }
        config_program::ProgramInfo::Cpp(program) => {
            let exe = compile_cpp(work_dir, &program.path, &program.compile_args)?;
            std::process::Command::new(exe)
        }
    };
    command.current_dir(work_dir);
    command.args(args);
    if input.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let started = Instant::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn `{}`", spec.label))?;
    if let Some(input) = input {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input)?;
        }
    }
    let output = child
        .controlled_with_output()
        .time_limit(std::time::Duration::from_secs_f64(spec.time_limit_secs))
        .memory_limit((spec.memory_limit_mb * 1024.0 * 1024.0) as usize)
        .terminate_for_timeout()
        .wait()?;
    let elapsed_ms = started.elapsed().as_millis();
    let Some(output) = output else {
        return Ok(RunResult {
            label: spec.label.clone(),
            ok: false,
            kind: "timeout".to_string(),
            exit_code: None,
            elapsed_ms,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            truncated_stdout: false,
            truncated_stderr: false,
        });
    };
    let (stdout_bytes, truncated_stdout) = limit_bytes(&output.stdout, output_limit_bytes);
    let (stderr_bytes, truncated_stderr) = limit_bytes(&output.stderr, output_limit_bytes);
    let stdout = decode_output(&stdout_bytes);
    let stderr = decode_output(&stderr_bytes);
    Ok(RunResult {
        label: spec.label.clone(),
        ok: output.status.success(),
        kind: if output.status.success() {
            "ok"
        } else {
            "runtime_error"
        }
        .to_string(),
        exit_code: output.status.code().map(|code| code as i32),
        elapsed_ms,
        stdout_bytes,
        stderr_bytes,
        stdout,
        stderr,
        truncated_stdout,
        truncated_stderr,
    })
}

fn compile_cpp(work_dir: &Path, source: &Path, compile_args: &[String]) -> Result<PathBuf> {
    let source = resolve_path(work_dir, source);
    let code =
        std::fs::read(&source).with_context(|| format!("failed to read {}", source.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&code);
    for arg in compile_args {
        hasher.update([0]);
        hasher.update(arg.as_bytes());
    }
    let digest = format!("{:x}", hasher.finalize());
    let cache_dir = work_dir
        .join(".cptool")
        .join("cache")
        .join("cpp")
        .join(digest);
    std::fs::create_dir_all(&cache_dir)?;
    let exe = cache_dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    if exe.exists() {
        return Ok(exe);
    }
    let _lock = acquire_compile_lock(&cache_dir, &exe)?;
    if exe.exists() {
        return Ok(exe);
    }
    let cached_source = cache_dir.join("main.cpp");
    std::fs::write(&cached_source, code)?;
    let temp_exe = cache_dir.join(if cfg!(windows) {
        format!("main-{}.tmp.exe", std::process::id())
    } else {
        format!("main-{}.tmp", std::process::id())
    });
    if temp_exe.exists() {
        std::fs::remove_file(&temp_exe)?;
    }
    let output = std::process::Command::new("g++")
        .current_dir(work_dir)
        .arg(&cached_source)
        .arg("-o")
        .arg(&temp_exe)
        .args(compile_args)
        .output()
        .context("failed to run g++")?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&temp_exe);
        anyhow::bail!(
            "compile failed for {}:\n{}",
            source.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::rename(&temp_exe, &exe)?;
    Ok(exe)
}

struct CompileLock {
    path: Option<PathBuf>,
}

impl Drop for CompileLock {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn acquire_compile_lock(cache_dir: &Path, exe: &Path) -> Result<CompileLock> {
    let lock_path = cache_dir.join("compile.lock");
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                writeln!(file, "pid={}", std::process::id())?;
                return Ok(CompileLock {
                    path: Some(lock_path),
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if exe.exists() {
                    return Ok(CompileLock { path: None });
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn absolutize_program_info(
    work_dir: &Path,
    info: &config_program::ProgramInfo,
) -> config_program::ProgramInfo {
    match info {
        config_program::ProgramInfo::Command(program) => {
            config_program::ProgramInfo::Command(config_program::CommandProgram {
                path: resolve_path(work_dir, &program.path),
                extra_args: program.extra_args.clone(),
            })
        }
        config_program::ProgramInfo::Python(program) => {
            config_program::ProgramInfo::Python(config_program::CommandProgram {
                path: resolve_path(work_dir, &program.path),
                extra_args: program.extra_args.clone(),
            })
        }
        config_program::ProgramInfo::Cpp(program) => {
            config_program::ProgramInfo::Cpp(config_program::CppProgram {
                path: resolve_path(work_dir, &program.path),
                compile_args: program.compile_args.clone(),
            })
        }
    }
}

fn resolve_path(work_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        work_dir.join(path)
    }
}

fn normalize_work_dir(work_dir: &Path) -> Result<PathBuf> {
    if work_dir.is_absolute() {
        Ok(work_dir.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(work_dir))
    }
}

fn case_file_stem(selector: &CaseSelector) -> String {
    format!("{}-{}", selector.bundle, selector.index)
}

fn limit_bytes(data: &[u8], limit: usize) -> (Vec<u8>, bool) {
    let truncated = data.len() > limit;
    let data = if truncated { &data[..limit] } else { data };
    (data.to_vec(), truncated)
}

fn decode_output(data: &[u8]) -> String {
    String::from_utf8_lossy(data)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
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

fn normalize_output(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_keeps_ascii_ids_predictable() {
        assert_eq!(slugify("My Problem 01").unwrap(), "my-problem-01");
        assert_eq!(slugify(" already_ok ").unwrap(), "already_ok");
        assert!(slugify("   ").is_err());
    }

    #[test]
    fn parse_case_selector_uses_zero_based_index() {
        let selector = parse_case_selector("s1[0]").unwrap();
        assert_eq!(selector.bundle, "s1");
        assert_eq!(selector.index, 0);
        assert!(parse_case_selector("s1").is_err());
        assert!(parse_case_selector("[0]").is_err());
    }

    #[test]
    fn normalize_output_trims_trailing_space_and_final_blankness() {
        assert_eq!(normalize_output("1  \r\n2\n\n"), "1\n2\n");
        assert_eq!(normalize_output("  \n"), "");
    }

    #[test]
    fn init_package_creates_cptool_layout() {
        let root = std::env::temp_dir().join(format!(
            "cptool-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let problem_dir = init_package(&root, "My Problem").unwrap();
        assert_eq!(problem_dir.file_name().unwrap(), "my-problem");
        assert!(problem_dir.join("problem.yaml").exists());
        assert!(problem_dir.join("src").join("std.cpp").exists());
        assert!(problem_dir.join("src").join("brute.cpp").exists());
        assert!(problem_dir.join("src").join("gen.cpp").exists());
        assert!(problem_dir.join("tests").join("failures").is_dir());
        assert!(problem_dir.join(".gitignore").exists());
        assert!(!problem_dir.join("quality_report.md").exists());
        assert!(!problem_dir.join("problem.md").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
