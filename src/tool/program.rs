use super::problem::resolve_path;
use super::schema::{
    CommandProgram, CppProgram, DEFAULT_MEMORY_LIMIT_MB, DEFAULT_TIME_LIMIT_SECS, Problem,
    ProgramInfo, RunResult,
};
use anyhow::{Context, Result};
use process_control::{ChildExt, Control};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(crate) struct ProgramSpec {
    pub(crate) label: String,
    pub(crate) info: ProgramInfo,
    pub(crate) time_limit_secs: f64,
    pub(crate) memory_limit_mb: f64,
}
pub(crate) fn resolve_run_spec(
    work_dir: &Path,
    problem: &Problem,
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

pub(crate) fn resolve_named_or_source(
    work_dir: &Path,
    problem: &Problem,
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
        Some("cpp") | Some("cc") | Some("cxx") => ProgramInfo::Cpp(CppProgram {
            path: source.to_path_buf(),
            compile_args: vec![
                "-O2".to_string(),
                "-std=c++20".to_string(),
                "-Wall".to_string(),
                "-Wextra".to_string(),
                "-pedantic".to_string(),
            ],
        }),
        Some("py") => ProgramInfo::Python(CommandProgram {
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

pub(crate) fn run_spec(
    work_dir: &Path,
    spec: &ProgramSpec,
    args: &[String],
    input: Option<&[u8]>,
    output_limit_bytes: usize,
) -> Result<RunResult> {
    let mut command = match &spec.info {
        ProgramInfo::Command(program) => {
            let mut command = std::process::Command::new(&program.path);
            command.args(&program.extra_args);
            command
        }
        ProgramInfo::Python(program) => {
            let mut command = std::process::Command::new(
                std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string()),
            );
            command
                .arg("-I")
                .arg(&program.path)
                .args(&program.extra_args);
            command
        }
        ProgramInfo::Cpp(program) => {
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

pub(crate) fn compile_cpp(
    work_dir: &Path,
    source: &Path,
    compile_args: &[String],
) -> Result<PathBuf> {
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
                if is_stale_compile_lock(&lock_path)? {
                    match std::fs::remove_file(&lock_path) {
                        Ok(()) => continue,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(err) => return Err(err.into()),
                    }
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(err) => return Err(err.into()),
        }
    }
}

pub(crate) fn is_stale_compile_lock(lock_path: &Path) -> Result<bool> {
    let content = match std::fs::read_to_string(lock_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    let Some(pid) = parse_lock_pid(&content) else {
        return Ok(true);
    };
    Ok(!process_exists(pid))
}

pub(crate) fn parse_lock_pid(content: &str) -> Option<u32> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .and_then(|pid| pid.trim().parse().ok())
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FI", &filter, "/NH"])
        .output()
    else {
        return true;
    };
    if !output.status.success() {
        return true;
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .any(|part| part == pid.to_string())
}

#[cfg(not(windows))]
fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(true)
}

pub(crate) fn absolutize_program_info(work_dir: &Path, info: &ProgramInfo) -> ProgramInfo {
    match info {
        ProgramInfo::Command(program) => ProgramInfo::Command(CommandProgram {
            path: resolve_path(work_dir, &program.path),
            extra_args: program.extra_args.clone(),
        }),
        ProgramInfo::Python(program) => ProgramInfo::Python(CommandProgram {
            path: resolve_path(work_dir, &program.path),
            extra_args: program.extra_args.clone(),
        }),
        ProgramInfo::Cpp(program) => ProgramInfo::Cpp(CppProgram {
            path: resolve_path(work_dir, &program.path),
            compile_args: program.compile_args.clone(),
        }),
    }
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
