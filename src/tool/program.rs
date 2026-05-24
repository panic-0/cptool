use super::problem::resolve_path;
use super::schema::{
    CommandProgram, CppProgram, DEFAULT_MEMORY_LIMIT_MB, DEFAULT_TIME_LIMIT_SECS, Problem,
    ProgramInfo, RunResult, default_compile_args,
};
use anyhow::{Context, Result};
use fs4::TryLockError;
use process_control::{ChildExt, Control};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const COMPILE_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
const COMPILE_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(25);

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
            compile_args: default_compile_args(),
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
    let stdout = child.stdout.take().context("child stdout was not piped")?;
    let stderr = child.stderr.take().context("child stderr was not piped")?;
    let stdout_reader = std::thread::spawn(move || read_limited_output(stdout, output_limit_bytes));
    let stderr_reader = std::thread::spawn(move || read_limited_output(stderr, output_limit_bytes));
    let stdin_writer = if let Some(input) = input {
        let mut stdin = child.stdin.take().context("child stdin was not piped")?;
        let input = input.to_vec();
        Some(std::thread::spawn(move || stdin.write_all(&input)))
    } else {
        None
    };
    let status = child
        .controlled()
        .time_limit(std::time::Duration::from_secs_f64(spec.time_limit_secs))
        .memory_limit((spec.memory_limit_mb * 1024.0 * 1024.0) as usize)
        .terminate_for_timeout()
        .wait()?;
    let elapsed_ms = started.elapsed().as_millis();

    let timed_out = status.is_none();
    if let Some(stdin_writer) = stdin_writer {
        let result = join_stdin_writer(stdin_writer);
        if !timed_out {
            result?;
        }
    }
    let stdout = join_output_reader(stdout_reader, "stdout");
    let stderr = join_output_reader(stderr_reader, "stderr");

    let Some(status) = status else {
        let stdout = stdout?;
        let stderr = stderr?;
        let stdout_bytes = stdout.bytes;
        let stderr_bytes = stderr.bytes;
        let stdout_text = decode_output(&stdout_bytes);
        let stderr_text = decode_output(&stderr_bytes);
        return Ok(RunResult {
            label: spec.label.clone(),
            ok: false,
            kind: "timeout".to_string(),
            exit_code: None,
            diagnostic: None,
            elapsed_ms,
            stdout_bytes,
            stderr_bytes,
            stdout: stdout_text,
            stderr: stderr_text,
            truncated_stdout: stdout.truncated,
            truncated_stderr: stderr.truncated,
        });
    };
    let stdout = stdout?;
    let stderr = stderr?;
    let stdout_bytes = stdout.bytes;
    let stderr_bytes = stderr.bytes;
    let stdout_text = decode_output(&stdout_bytes);
    let stderr_text = decode_output(&stderr_bytes);
    let exit_code = status.code().map(|code| code as i32);
    Ok(RunResult {
        label: spec.label.clone(),
        ok: status.success(),
        kind: if status.success() {
            "ok"
        } else {
            "runtime_error"
        }
        .to_string(),
        exit_code,
        diagnostic: if status.success() {
            None
        } else {
            runtime_exit_diagnostic(exit_code)
        },
        elapsed_ms,
        stdout_bytes,
        stderr_bytes,
        stdout: stdout_text,
        stderr: stderr_text,
        truncated_stdout: stdout.truncated,
        truncated_stderr: stderr.truncated,
    })
}

pub(crate) fn compile_cpp(
    work_dir: &Path,
    source: &Path,
    compile_args: &[String],
) -> Result<PathBuf> {
    let source = resolve_path(work_dir, source);
    let effective_compile_args = effective_cpp_compile_args(&source, compile_args);
    let cache_inputs = collect_cpp_cache_inputs(&source)?;
    let code = cache_inputs
        .first()
        .context("C++ cache input list was unexpectedly empty")?
        .bytes
        .clone();
    let toolchain = CppToolchain::detect();
    let digest = cpp_cache_key(&cache_inputs, &effective_compile_args, &toolchain);
    let cache_dir = work_dir
        .join(".cptool")
        .join("cache")
        .join("cpp")
        .join(&digest);
    std::fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create C++ cache dir {}", cache_dir.display()))?;
    let exe = cache_dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    if exe.exists() {
        return Ok(exe);
    }
    let _lock = acquire_compile_lock(&cache_dir, &exe)?;
    if exe.exists() {
        return Ok(exe);
    }
    let cached_source = cache_dir.join("main.cpp");
    std::fs::write(&cached_source, code).with_context(|| {
        format!(
            "failed to write cached C++ source {}",
            cached_source.display()
        )
    })?;
    let temp_exe = cache_dir.join(if cfg!(windows) {
        format!("main-{}.tmp.exe", std::process::id())
    } else {
        format!("main-{}.tmp", std::process::id())
    });
    if temp_exe.exists() {
        std::fs::remove_file(&temp_exe)
            .with_context(|| format!("failed to remove stale temp exe {}", temp_exe.display()))?;
    }
    let diagnostics = cpp_compile_diagnostics(&digest, &exe, &effective_compile_args, &toolchain);
    let output = std::process::Command::new("g++")
        .current_dir(work_dir)
        .arg(&cached_source)
        .arg("-o")
        .arg(&temp_exe)
        .args(&effective_compile_args)
        .output()
        .with_context(|| format!("failed to run g++\n{}", diagnostics.render()))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&temp_exe);
        anyhow::bail!(
            "compile failed for {}:\n{}\n{}",
            source.display(),
            diagnostics.render(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::rename(&temp_exe, &exe).with_context(|| {
        format!(
            "failed to move compiled temp exe {} to {}",
            temp_exe.display(),
            exe.display()
        )
    })?;
    Ok(exe)
}

fn effective_cpp_compile_args(source: &Path, compile_args: &[String]) -> Vec<String> {
    let mut args = compile_args.to_vec();
    if cfg!(windows) && !args.iter().any(|arg| arg == "-static") {
        args.push("-static".to_string());
    }
    if let Some(parent) = source.parent() {
        args.push("-I".to_string());
        args.push(parent.to_string_lossy().into_owned());
    }
    args
}

#[derive(Clone, Debug)]
struct CppCacheInput {
    path: PathBuf,
    bytes: Vec<u8>,
}

fn collect_cpp_cache_inputs(source: &Path) -> Result<Vec<CppCacheInput>> {
    let source = source
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", source.display()))?;
    let source_root = source
        .parent()
        .context("C++ source path has no parent directory")?
        .canonicalize()
        .with_context(|| {
            format!(
                "failed to resolve source directory for {}",
                source.display()
            )
        })?;
    let mut visited = HashSet::new();
    let mut inputs = Vec::new();
    collect_cpp_cache_inputs_recursive(&source, &source_root, &mut visited, &mut inputs)?;
    Ok(inputs)
}

fn collect_cpp_cache_inputs_recursive(
    path: &Path,
    source_root: &Path,
    visited: &mut HashSet<PathBuf>,
    inputs: &mut Vec<CppCacheInput>,
) -> Result<()> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    if !visited.insert(path.clone()) {
        return Ok(());
    }

    let bytes =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let includes = parse_quoted_includes(&bytes);
    inputs.push(CppCacheInput {
        path: path.clone(),
        bytes,
    });

    let current_dir = path.parent().unwrap_or(source_root);
    for include in includes {
        if let Some(include_path) = resolve_local_include(&include, current_dir, source_root) {
            collect_cpp_cache_inputs_recursive(&include_path, source_root, visited, inputs)?;
        }
    }
    Ok(())
}

fn parse_quoted_includes(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let directive = line.strip_prefix('#')?.trim_start();
            let rest = directive.strip_prefix("include")?;
            if rest
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                return None;
            }
            let rest = rest.trim_start().strip_prefix('"')?;
            let end = rest.find('"')?;
            let include = &rest[..end];
            if include.is_empty() {
                None
            } else {
                Some(include.to_string())
            }
        })
        .collect()
}

fn resolve_local_include(include: &str, current_dir: &Path, source_root: &Path) -> Option<PathBuf> {
    let include = Path::new(include);
    for base in [current_dir, source_root] {
        let candidate = if include.is_absolute() {
            include.to_path_buf()
        } else {
            base.join(include)
        };
        let Ok(candidate) = candidate.canonicalize() else {
            continue;
        };
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[derive(Clone, Debug)]
struct CppToolchain {
    gpp_path: String,
    gpp_version_first_line: Option<String>,
}

impl CppToolchain {
    fn detect() -> Self {
        Self {
            gpp_path: resolve_command_path("g++").unwrap_or_else(|| "g++".to_string()),
            gpp_version_first_line: command_version_first_line("g++"),
        }
    }
}

fn cpp_cache_key(
    inputs: &[CppCacheInput],
    compile_args: &[String],
    toolchain: &CppToolchain,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"cptool-cpp-cache-v2");
    hasher.update([0]);
    hasher.update(toolchain.gpp_path.as_bytes());
    hasher.update([0]);
    hasher.update(
        toolchain
            .gpp_version_first_line
            .as_deref()
            .unwrap_or("<unavailable>")
            .as_bytes(),
    );
    for arg in compile_args {
        hasher.update([0]);
        hasher.update(arg.as_bytes());
    }
    for input in inputs {
        hasher.update([0]);
        hasher.update(input.path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(&input.bytes);
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Clone, Debug)]
struct CppCompileDiagnostics {
    gpp_path: String,
    gpp_version_first_line: Option<String>,
    compile_args: Vec<String>,
    has_static: bool,
    cache_key: String,
    exe_path: PathBuf,
}

impl CppCompileDiagnostics {
    fn render(&self) -> String {
        let version = self
            .gpp_version_first_line
            .as_deref()
            .unwrap_or("<unavailable>");
        format!(
            "g++ path: {}\ng++ version: {}\ncompile args: {}\ncontains -static: {}\ncache key: {}\ncache exe: {}",
            self.gpp_path,
            version,
            render_args(&self.compile_args),
            self.has_static,
            self.cache_key,
            self.exe_path.display()
        )
    }
}

fn cpp_compile_diagnostics(
    cache_key: &str,
    exe_path: &Path,
    compile_args: &[String],
    toolchain: &CppToolchain,
) -> CppCompileDiagnostics {
    CppCompileDiagnostics {
        gpp_path: toolchain.gpp_path.clone(),
        gpp_version_first_line: toolchain.gpp_version_first_line.clone(),
        compile_args: compile_args.to_vec(),
        has_static: compile_args.iter().any(|arg| arg == "-static"),
        cache_key: cache_key.to_string(),
        exe_path: exe_path.to_path_buf(),
    }
}

fn command_version_first_line(command: &str) -> Option<String> {
    let output = std::process::Command::new(command)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    decode_output(&output.stdout)
        .lines()
        .next()
        .map(str::to_string)
}

fn resolve_command_path(command: &str) -> Option<String> {
    let finder = if cfg!(windows) { "where" } else { "which" };
    let output = std::process::Command::new(finder)
        .arg(command)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    decode_output(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn render_args(args: &[String]) -> String {
    if args.is_empty() {
        "<none>".to_string()
    } else {
        args.join(" ")
    }
}

fn runtime_exit_diagnostic(exit_code: Option<i32>) -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    match exit_code? {
        -1073741511 => Some(
            "Windows NTSTATUS 0xC0000139: entry point or DLL load failure. Check that required runtime/DLL files are on PATH and match the compiled architecture."
                .to_string(),
        ),
        -1073741819 => Some(
            "Windows NTSTATUS 0xC0000005: access violation. The program tried to read, write, or execute invalid memory."
                .to_string(),
        ),
        _ => None,
    }
}

#[derive(Debug)]
struct CompileLock {
    _file: Option<File>,
}

fn acquire_compile_lock(cache_dir: &Path, exe: &Path) -> Result<CompileLock> {
    acquire_compile_lock_with_timeout(cache_dir, exe, COMPILE_LOCK_TIMEOUT)
}

fn acquire_compile_lock_with_timeout(
    cache_dir: &Path,
    exe: &Path,
    timeout: Duration,
) -> Result<CompileLock> {
    let lock_path = cache_dir.join("compile.lock");
    let start = Instant::now();
    loop {
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("failed to open compile lock {}", lock_path.display()))?;
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => {
                file.set_len(0).with_context(|| {
                    format!("failed to truncate compile lock {}", lock_path.display())
                })?;
                writeln!(file, "pid={}", std::process::id()).with_context(|| {
                    format!("failed to write compile lock {}", lock_path.display())
                })?;
                return Ok(CompileLock { _file: Some(file) });
            }
            Err(TryLockError::WouldBlock) => {
                if exe.exists() {
                    return Ok(CompileLock { _file: None });
                }
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    anyhow::bail!(
                        "timed out after {:.3}s waiting for C++ compile lock {} (cache_dir={} exe={})",
                        timeout.as_secs_f64(),
                        lock_path.display(),
                        cache_dir.display(),
                        exe.display()
                    );
                }
                std::thread::sleep(
                    timeout
                        .saturating_sub(elapsed)
                        .min(COMPILE_LOCK_POLL_INTERVAL),
                );
            }
            Err(TryLockError::Error(err)) => {
                return Err(err).with_context(|| {
                    format!("failed to lock compile lock {}", lock_path.display())
                });
            }
        }
    }
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
struct LimitedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_limited_output<R: Read>(mut reader: R, limit: usize) -> std::io::Result<LimitedOutput> {
    let mut bytes = Vec::with_capacity(limit.min(8192));
    let mut truncated = false;
    let mut buffer = [0; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        if remaining >= read {
            bytes.extend_from_slice(&buffer[..read]);
        } else {
            bytes.extend_from_slice(&buffer[..remaining]);
            truncated = true;
        }
    }
    Ok(LimitedOutput { bytes, truncated })
}

fn join_output_reader(
    reader: JoinHandle<std::io::Result<LimitedOutput>>,
    pipe_name: &str,
) -> Result<LimitedOutput> {
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("{pipe_name} reader thread panicked"))?
        .with_context(|| format!("failed to read child {pipe_name}"))
}

fn join_stdin_writer(writer: JoinHandle<std::io::Result<()>>) -> Result<()> {
    match writer
        .join()
        .map_err(|_| anyhow::anyhow!("stdin writer thread panicked"))?
    {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(err).context("failed to write child stdin"),
    }
}

fn decode_output(data: &[u8]) -> String {
    String::from_utf8_lossy(data)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::python_available;
    use crate::tool::temp_test_dir;
    use std::io::Cursor;
    use std::sync::Mutex;

    static RUN_SPEC_SUBPROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn limited_output_keeps_prefix_and_drains_reader() {
        let output = read_limited_output(Cursor::new(b"abcdef"), 3).unwrap();

        assert_eq!(output.bytes, b"abc");
        assert!(output.truncated);
    }

    #[test]
    fn limited_output_limit_zero_marks_non_empty_output_truncated() {
        let output = read_limited_output(Cursor::new(b"x"), 0).unwrap();

        assert!(output.bytes.is_empty());
        assert!(output.truncated);
    }

    #[test]
    fn run_spec_handles_large_stdin_and_stdout_concurrently() {
        if !python_available() {
            return;
        }
        let _guard = RUN_SPEC_SUBPROCESS_TEST_LOCK.lock().unwrap();
        let root = temp_test_dir("cptool-runner-pipes");
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("pipe_pressure.py");
        std::fs::write(
            &script,
            r#"
import sys

sys.stdout.buffer.write(b"x" * 1048576)
sys.stdout.buffer.flush()
data = sys.stdin.buffer.read()
sys.stdout.buffer.write(str(len(data)).encode("ascii"))
"#,
        )
        .unwrap();
        let spec = ProgramSpec {
            label: "pipe_pressure".to_string(),
            info: ProgramInfo::Python(CommandProgram {
                path: script,
                extra_args: Vec::new(),
            }),
            time_limit_secs: 20.0,
            memory_limit_mb: 512.0,
        };
        let input = vec![b'i'; 1024 * 1024];

        let result = run_spec(&root, &spec, &[], Some(&input), 32).unwrap();

        assert!(result.ok, "{}", result.status_line());
        assert_eq!(result.stdout_bytes, vec![b'x'; 32]);
        assert!(result.truncated_stdout);
        assert!(!result.truncated_stderr);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn run_spec_timeout_keeps_limited_stdout_and_stderr() {
        if !python_available() {
            return;
        }
        let _guard = RUN_SPEC_SUBPROCESS_TEST_LOCK.lock().unwrap();
        let root = temp_test_dir("cptool-runner-timeout-output");
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("timeout_output.py");
        std::fs::write(
            &script,
            r#"
import sys
import time

sys.stdout.buffer.write(b"stdout-before-timeout")
sys.stdout.buffer.flush()
sys.stderr.buffer.write(b"stderr-before-timeout")
sys.stderr.buffer.flush()
time.sleep(60)
"#,
        )
        .unwrap();
        let spec = ProgramSpec {
            label: "timeout_output".to_string(),
            info: ProgramInfo::Python(CommandProgram {
                path: script,
                extra_args: Vec::new(),
            }),
            time_limit_secs: 10.0,
            memory_limit_mb: 512.0,
        };

        let result = run_spec(&root, &spec, &[], None, 6).unwrap();

        assert!(!result.ok);
        assert_eq!(result.kind, "timeout");
        assert_eq!(result.exit_code, None);
        assert_eq!(result.stdout_bytes, b"stdout");
        assert_eq!(result.stderr_bytes, b"stderr");
        assert_eq!(result.stdout, "stdout");
        assert_eq!(result.stderr, "stderr");
        assert!(result.truncated_stdout);
        assert!(result.truncated_stderr);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cpp_compile_diagnostics_render_key_toolchain_and_cache_context() {
        let args = vec![
            "-O2".to_string(),
            "-std=c++20".to_string(),
            "-static".to_string(),
        ];
        let diagnostics = CppCompileDiagnostics {
            gpp_path: "C:\\tools\\mingw\\bin\\g++.exe".to_string(),
            gpp_version_first_line: Some("g++ (Rev1) 13.2.0".to_string()),
            compile_args: args.clone(),
            has_static: args.iter().any(|arg| arg == "-static"),
            cache_key: cpp_cache_key(
                &[CppCacheInput {
                    path: PathBuf::from("main.cpp"),
                    bytes: b"int main(){}".to_vec(),
                }],
                &args,
                &CppToolchain {
                    gpp_path: "C:\\tools\\mingw\\bin\\g++.exe".to_string(),
                    gpp_version_first_line: Some("g++ (Rev1) 13.2.0".to_string()),
                },
            ),
            exe_path: PathBuf::from("D:\\work\\.cptool\\cache\\cpp\\abc\\main.exe"),
        };

        let rendered = diagnostics.render();

        assert!(rendered.contains("g++ path: C:\\tools\\mingw\\bin\\g++.exe"));
        assert!(rendered.contains("g++ version: g++ (Rev1) 13.2.0"));
        assert!(rendered.contains("compile args: -O2 -std=c++20 -static"));
        assert!(rendered.contains("contains -static: true"));
        assert!(rendered.contains("cache key: "));
        assert!(rendered.contains("cache exe: "));
    }

    #[test]
    fn effective_cpp_compile_args_adds_windows_static_linking_once() {
        let source = Path::new("src/main.cpp");
        let args = effective_cpp_compile_args(source, &["-O2".to_string()]);

        if cfg!(windows) {
            assert!(args.iter().any(|arg| arg == "-static"));
        } else {
            assert!(!args.iter().any(|arg| arg == "-static"));
        }

        let explicit_static =
            effective_cpp_compile_args(source, &["-O2".to_string(), "-static".to_string()]);
        assert_eq!(
            explicit_static
                .iter()
                .filter(|arg| arg.as_str() == "-static")
                .count(),
            1
        );
    }

    #[test]
    fn cpp_compile_adds_source_directory_to_include_path() {
        if command_version_first_line("g++").is_none() {
            return;
        }
        let root = temp_test_dir("cptool-cpp-include-path");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("common.hpp"), "#define ANSWER 42\n").unwrap();
        let main = src_dir.join("main.cpp");
        std::fs::write(
            &main,
            "#include \"common.hpp\"\n#include <iostream>\nint main(){ std::cout << ANSWER << '\\n'; }\n",
        )
        .unwrap();

        let exe = compile_cpp(&root, &main, &default_compile_args()).unwrap();

        assert!(exe.exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cpp_compile_cache_key_includes_local_header_contents() {
        if command_version_first_line("g++").is_none() {
            return;
        }
        let root = temp_test_dir("cptool-cpp-header-cache-key");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let header = src_dir.join("common.hpp");
        std::fs::write(&header, "#define ANSWER 1\n").unwrap();
        let main = src_dir.join("main.cpp");
        std::fs::write(
            &main,
            "#include \"common.hpp\"\n#include <iostream>\nint main(){ std::cout << ANSWER << '\\n'; }\n",
        )
        .unwrap();

        let first_exe = compile_cpp(&root, &main, &default_compile_args()).unwrap();
        let first_output = std::process::Command::new(&first_exe).output().unwrap();
        assert_eq!(decode_output(&first_output.stdout), "1\n");

        std::fs::write(&header, "#define ANSWER 2\n").unwrap();
        let second_exe = compile_cpp(&root, &main, &default_compile_args()).unwrap();
        let second_output = std::process::Command::new(&second_exe).output().unwrap();

        assert_ne!(first_exe, second_exe);
        assert_eq!(decode_output(&second_output.stdout), "2\n");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cpp_compile_cache_key_includes_relative_headers_outside_source_dir() {
        if command_version_first_line("g++").is_none() {
            return;
        }
        let root = temp_test_dir("cptool-cpp-external-header-cache-key");
        let src_dir = root.join("src");
        let shared_dir = root.join("shared");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&shared_dir).unwrap();
        let shared = shared_dir.join("common.hpp");
        std::fs::write(&shared, "#define ANSWER 1\n").unwrap();
        std::fs::write(
            src_dir.join("bridge.hpp"),
            "#pragma once\n#include \"../shared/common.hpp\"\n",
        )
        .unwrap();
        let main = src_dir.join("main.cpp");
        std::fs::write(
            &main,
            "#include \"bridge.hpp\"\n#include <iostream>\nint main(){ std::cout << ANSWER << '\\n'; }\n",
        )
        .unwrap();

        let first_exe = compile_cpp(&root, &main, &default_compile_args()).unwrap();
        let first_output = std::process::Command::new(&first_exe).output().unwrap();
        assert_eq!(decode_output(&first_output.stdout), "1\n");

        std::fs::write(&shared, "#define ANSWER 2\n").unwrap();
        let second_exe = compile_cpp(&root, &main, &default_compile_args()).unwrap();
        let second_output = std::process::Command::new(&second_exe).output().unwrap();

        assert_ne!(first_exe, second_exe);
        assert_eq!(decode_output(&second_output.stdout), "2\n");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compile_lock_times_out_with_context() {
        let root = temp_test_dir("cptool-compile-lock-timeout");
        let cache_dir = root.join(".cptool").join("cache").join("cpp").join("held");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let lock_path = cache_dir.join("compile.lock");
        let lock_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        fs4::FileExt::try_lock(&lock_file).unwrap();

        let err = acquire_compile_lock_with_timeout(
            &cache_dir,
            &cache_dir.join(if cfg!(windows) { "main.exe" } else { "main" }),
            Duration::from_millis(50),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("timed out"));
        assert!(err.contains("compile.lock"));
        assert!(err.contains("cache_dir="));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cpp_compile_cache_lock_allows_concurrent_same_source_compile() {
        if command_version_first_line("g++").is_none() {
            return;
        }
        let root = temp_test_dir("cptool-cpp-concurrent-cache");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let main = src_dir.join("main.cpp");
        std::fs::write(
            &main,
            "#include <iostream>\nint main(){ std::cout << 42 << '\\n'; }\n",
        )
        .unwrap();

        let first_root = root.clone();
        let first_main = main.clone();
        let first = std::thread::spawn(move || {
            compile_cpp(&first_root, &first_main, &default_compile_args())
        });
        let second_root = root.clone();
        let second_main = main.clone();
        let second = std::thread::spawn(move || {
            compile_cpp(&second_root, &second_main, &default_compile_args())
        });

        let first_exe = first.join().unwrap().unwrap();
        let second_exe = second.join().unwrap().unwrap();

        assert_eq!(first_exe, second_exe);
        assert!(first_exe.exists());
        let output = std::process::Command::new(&first_exe).output().unwrap();
        assert_eq!(decode_output(&output.stdout), "42\n");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn runtime_exit_diagnostic_names_common_windows_ntstatus_codes() {
        let dll_hint = runtime_exit_diagnostic(Some(-1073741511)).unwrap();
        assert!(dll_hint.contains("0xC0000139"));
        assert!(dll_hint.contains("DLL"));

        let access_violation_hint = runtime_exit_diagnostic(Some(-1073741819)).unwrap();
        assert!(access_violation_hint.contains("0xC0000005"));
        assert!(access_violation_hint.contains("access violation"));

        assert!(runtime_exit_diagnostic(Some(1)).is_none());
    }
}
