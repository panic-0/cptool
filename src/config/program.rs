use crate::core::program as core_problem;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandProgram {
    pub path: std::path::PathBuf,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl std::fmt::Display for CommandProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (extra args: `{}`)",
            self.path.display(),
            self.extra_args.join(" ")
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CppProgram {
    pub path: std::path::PathBuf,
    #[serde(default = "default_compile_args")]
    pub compile_args: Vec<String>,
}

fn default_compile_args() -> Vec<String> {
    vec![
        "-O2".to_string(),
        "-std=c++20".to_string(),
        "-Wall".to_string(),
        "-Wextra".to_string(),
        "-pedantic".to_string(),
    ]
}

impl std::fmt::Display for CppProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (compile args: `{}`)",
            self.path.display(),
            self.compile_args.join(" ")
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProgramInfo {
    #[serde(rename = "command")]
    Command(CommandProgram),
    #[serde(rename = "cpp")]
    Cpp(CppProgram),
    #[serde(rename = "python")]
    Python(CommandProgram),
}

impl std::fmt::Display for ProgramInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgramInfo::Command(program) => write!(f, "Command {}", program),
            ProgramInfo::Cpp(program) => write!(f, "Cpp {}", program),
            ProgramInfo::Python(program) => write!(f, "Python {}", program),
        }
    }
}

impl ProgramInfo {
    pub fn generate(
        &self,
        name: &str,
        output_dir: &std::path::Path,
    ) -> Result<core_problem::ProgramInfo> {
        match &self {
            ProgramInfo::Command(CommandProgram { path, extra_args }) => Ok(
                core_problem::ProgramInfo::Command(core_problem::CommandProgram {
                    path: path.into(),
                    extra_args: extra_args.clone(),
                }),
            ),
            ProgramInfo::Python(CommandProgram { path, extra_args }) => Ok(
                core_problem::ProgramInfo::Python(core_problem::CommandProgram {
                    path: path.into(),
                    extra_args: extra_args.clone(),
                }),
            ),
            ProgramInfo::Cpp(CppProgram { path, compile_args }) => {
                let exe_name = if cfg!(windows) {
                    format!("{}.exe", name)
                } else {
                    name.to_string()
                };

                let exe_path = output_dir.join(exe_name);
                if exe_path.exists() {
                    std::fs::remove_file(&exe_path)?;
                }
                let output = std::process::Command::new("g++")
                    .arg("-o")
                    .arg(exe_path.clone())
                    .args(compile_args)
                    .arg(path)
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow::anyhow!(
                        "compile error: {}\n{}",
                        &self,
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                println!("compile success: {}", &self);

                Ok(core_problem::ProgramInfo::Cpp(core_problem::CppProgram {
                    path: exe_path,
                    source_path: path.into(),
                    compile_args: compile_args.clone(),
                }))
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Program {
    pub info: ProgramInfo,
    pub time_limit_secs: f64,
    pub memory_limit_mb: f64,
}

impl std::fmt::Display for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (time limit: {}s, memory limit: {}MB)",
            self.info, self.time_limit_secs, self.memory_limit_mb
        )
    }
}

impl Program {
    pub fn generate(
        &self,
        name: &str,
        output_dir: &std::path::Path,
    ) -> Result<core_problem::Program> {
        Ok(core_problem::Program {
            info: self.info.generate(name, output_dir)?,
            time_limit_secs: self.time_limit_secs,
            memory_limit_mb: self.memory_limit_mb,
        })
    }
}
