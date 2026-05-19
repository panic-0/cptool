use super::problem::resolve_path;
use super::schema::{
    CommandProgram, CppProgram, DEFAULT_MEMORY_LIMIT_MB, DEFAULT_TIME_LIMIT_SECS, Problem,
    ProgramInfo, RunResult,
};
use anyhow::{Context, Result};
use process_control::{ChildExt, Control};
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread::JoinHandle;
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
        return Ok(RunResult {
            label: spec.label.clone(),
            ok: false,
            kind: "timeout".to_string(),
            exit_code: None,
            diagnostic: None,
            elapsed_ms,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            truncated_stdout: false,
            truncated_stderr: false,
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
    let code =
        std::fs::read(&source).with_context(|| format!("failed to read {}", source.display()))?;
    let digest = cpp_cache_key(&code, compile_args);
    let cache_dir = work_dir
        .join(".cptool")
        .join("cache")
        .join("cpp")
        .join(&digest);
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
    let diagnostics = cpp_compile_diagnostics(&digest, &exe, compile_args);
    let output = std::process::Command::new("g++")
        .current_dir(work_dir)
        .arg(&cached_source)
        .arg("-o")
        .arg(&temp_exe)
        .args(compile_args)
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
    std::fs::rename(&temp_exe, &exe)?;
    Ok(exe)
}

fn cpp_cache_key(code: &[u8], compile_args: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code);
    for arg in compile_args {
        hasher.update([0]);
        hasher.update(arg.as_bytes());
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
) -> CppCompileDiagnostics {
    CppCompileDiagnostics {
        gpp_path: resolve_command_path("g++").unwrap_or_else(|| "g++".to_string()),
        gpp_version_first_line: command_version_first_line("g++"),
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
    use std::io::Cursor;

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
            time_limit_secs: 5.0,
            memory_limit_mb: 128.0,
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
            cache_key: cpp_cache_key(b"int main(){}", &args),
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

    fn python_available() -> bool {
        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string());
        std::process::Command::new(python)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn temp_test_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
