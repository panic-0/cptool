use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct CommandProgram {
    pub path: std::path::PathBuf,
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

#[derive(Clone, Debug)]
pub struct CppProgram {
    pub path: std::path::PathBuf,
    pub source_path: std::path::PathBuf,
    pub compile_args: Vec<String>,
}

impl std::fmt::Display for CppProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (compile args: `{}`)",
            self.source_path.display(),
            self.compile_args.join(" ")
        )
    }
}

#[derive(Clone, Debug)]
pub enum ProgramInfo {
    Command(CommandProgram),
    Cpp(CppProgram),
    Python(CommandProgram),
}

impl std::fmt::Display for ProgramInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgramInfo::Command(program) => write!(f, "Command {program}"),
            ProgramInfo::Cpp(program) => write!(f, "Cpp {program}"),
            ProgramInfo::Python(program) => write!(f, "Python {program}"),
        }
    }
}

#[derive(Clone, Debug)]
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
    fn execute_command(&self, command: &mut std::process::Command) -> Result<()> {
        use process_control::{ChildExt, Control};
        let child = command.spawn()?;
        let output = child
            .controlled_with_output()
            .time_limit(std::time::Duration::from_secs_f64(self.time_limit_secs))
            .memory_limit((self.memory_limit_mb * 1024.0 * 1024.0) as usize)
            .terminate_for_timeout()
            .wait()?
            .ok_or_else(|| anyhow::anyhow!("time limit exceeded: {}", &self))?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("runtime error: {}", &self));
        }
        Ok(())
    }

    pub fn execute(
        &self,
        args: Vec<String>,
        input: Option<std::fs::File>,
        output: Option<std::fs::File>,
    ) -> Result<()> {
        match &self.info {
            ProgramInfo::Command(CommandProgram { path, extra_args }) => {
                let mut command = std::process::Command::new(path);
                if let Some(input) = input {
                    command.stdin(input);
                }
                if let Some(output) = output {
                    command.stdout(output);
                }
                command.args(extra_args).args(args.clone());

                self.execute_command(&mut command)
            }
            ProgramInfo::Cpp(CppProgram { path, .. }) => {
                let mut command = std::process::Command::new(path);
                if let Some(input) = input {
                    command.stdin(input);
                }
                if let Some(output) = output {
                    command.stdout(output);
                }
                command.args(args.clone());

                self.execute_command(&mut command)
            }
            ProgramInfo::Python(CommandProgram { path, extra_args }) => {
                let mut command = std::process::Command::new(
                    std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string()),
                );
                command.arg("-I").arg(path);
                if let Some(input) = input {
                    command.stdin(input);
                }
                if let Some(output) = output {
                    command.stdout(output);
                }
                command.args(extra_args).args(args.clone());

                self.execute_command(&mut command)
            }
        }
        .with_context(|| format!("failed to execute {} (args: `{}`)", self, args.join(" ")))
    }
}
