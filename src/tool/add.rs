use super::schema::{
    CommandProgram, CppProgram, OutputConfig, Problem, Program, ProgramInfo, TestBundle, TestCase,
    TestTask, TestTaskType,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const DEFAULT_ADD_TIME_LIMIT_SECS: f64 = 3.0;
const DEFAULT_ADD_MEMORY_LIMIT_MB: f64 = 512.0;

mod generated_builtin_checkers {
    include!(concat!(env!("OUT_DIR"), "/builtin_checkers.rs"));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddProgramKind {
    Cpp,
    Python,
    Command,
}

#[derive(Clone, Debug)]
pub struct AddProgramOptions {
    pub work_dir: PathBuf,
    pub name: String,
    pub kind: Option<AddProgramKind>,
    pub path: Option<PathBuf>,
    pub time_limit_secs: Option<f64>,
    pub memory_limit_mb: Option<f64>,
    pub compile_args: Vec<String>,
    pub replace: bool,
}

#[derive(Clone, Debug)]
pub struct AddBundleOptions {
    pub work_dir: PathBuf,
    pub name: String,
    pub generator: Option<String>,
    pub cases: Vec<Vec<String>>,
    pub replace: bool,
}

#[derive(Clone, Debug)]
pub struct AddTaskOptions {
    pub work_dir: PathBuf,
    pub name: String,
    pub score: Option<f64>,
    pub task_type: TestTaskType,
    pub bundles: Vec<String>,
    pub dependencies: Vec<String>,
    pub expect_pass: Vec<String>,
    pub expect_fail: Vec<String>,
    pub replace: bool,
}

#[derive(Clone, Debug)]
pub struct AddValidatorOptions {
    pub work_dir: PathBuf,
    pub name: String,
    pub time_limit_secs: Option<f64>,
    pub memory_limit_mb: Option<f64>,
    pub compile_args: Vec<String>,
    pub replace: bool,
}

#[derive(Clone, Debug)]
pub struct AddCheckerOptions {
    pub work_dir: PathBuf,
    pub name: String,
    pub builtin: Option<String>,
    pub time_limit_secs: Option<f64>,
    pub memory_limit_mb: Option<f64>,
    pub compile_args: Vec<String>,
    pub replace: bool,
}

pub fn add_program(options: AddProgramOptions) -> Result<()> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let mut problem = read_problem(&work_dir)?;
    if problem.programs.contains_key(&options.name) && !options.replace {
        anyhow::bail!(
            "program `{}` already exists; pass --replace to overwrite",
            options.name
        );
    }

    let (kind, path) = resolve_program_path(
        &work_dir,
        &options.name,
        options.kind,
        options.path.as_deref(),
    )?;

    let program = program_from_parts(
        kind,
        path,
        options.time_limit_secs,
        options.memory_limit_mb,
        &options.compile_args,
        &problem.cpp_compile_args,
    );
    problem.programs.insert(options.name, program);
    write_problem(&work_dir, &problem)
}

pub fn add_bundle(options: AddBundleOptions) -> Result<()> {
    if options.cases.is_empty() {
        anyhow::bail!("add bundle requires at least one --case");
    }
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let mut problem = read_problem(&work_dir)?;
    if problem.test.bundles.contains_key(&options.name) && !options.replace {
        anyhow::bail!(
            "bundle `{}` already exists; pass --replace to overwrite",
            options.name
        );
    }
    let generator = options.generator.unwrap_or_else(|| "gen".to_string());
    let cases = options
        .cases
        .into_iter()
        .map(|args| TestCase {
            generator_name: generator.clone(),
            args,
        })
        .collect();
    problem
        .test
        .bundles
        .insert(options.name, TestBundle { cases });
    write_problem(&work_dir, &problem)
}

pub fn add_task(options: AddTaskOptions) -> Result<()> {
    if options.bundles.is_empty() {
        anyhow::bail!("add task requires at least one --bundle");
    }
    if options.score.is_none() && options.expect_pass.is_empty() && options.expect_fail.is_empty() {
        anyhow::bail!("verify-only task requires --pass or --fail");
    }
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let mut problem = read_problem(&work_dir)?;
    if let Some(index) = problem
        .test
        .tasks
        .iter()
        .position(|task| task.name == options.name)
    {
        if !options.replace {
            anyhow::bail!(
                "task `{}` already exists; pass --replace to overwrite",
                options.name
            );
        }
        problem.test.tasks.remove(index);
    }
    problem.test.tasks.push(TestTask {
        name: options.name,
        score: options.score,
        task_type: options.score.map(|_| options.task_type),
        bundles: options.bundles,
        cases: Vec::new(),
        dependencies: options.dependencies,
        pass_programs: options.expect_pass,
        fail_programs: options.expect_fail,
    });
    write_problem(&work_dir, &problem)
}

pub fn add_validator(options: AddValidatorOptions) -> Result<()> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let mut problem = read_problem(&work_dir)?;
    if problem.validator_name.is_some() && !options.replace {
        anyhow::bail!("validator is already configured; pass --replace to overwrite");
    }

    if problem.programs.contains_key(&options.name) && !options.replace {
        problem.validator_name = Some(options.name);
        problem.validator_omitted_reason = None;
        return write_problem(&work_dir, &problem);
    }

    let (kind, source_path) = resolve_program_path(&work_dir, &options.name, None, None)?;

    problem.programs.insert(
        options.name.clone(),
        program_from_parts(
            kind,
            source_path,
            options.time_limit_secs,
            options.memory_limit_mb,
            &options.compile_args,
            &problem.cpp_compile_args,
        ),
    );
    problem.validator_name = Some(options.name);
    problem.validator_omitted_reason = None;
    write_problem(&work_dir, &problem)
}

pub fn add_checker(options: AddCheckerOptions) -> Result<()> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let mut problem = read_problem(&work_dir)?;
    if problem.checker_name.is_some() && !options.replace {
        anyhow::bail!("checker is already configured; pass --replace to overwrite");
    }

    if options.builtin.is_none() && problem.programs.contains_key(&options.name) && !options.replace
    {
        problem.checker_name = Some(options.name);
        return write_problem(&work_dir, &problem);
    }

    if problem.programs.contains_key(&options.name) && !options.replace {
        anyhow::bail!(
            "program `{}` already exists; pass --replace to overwrite",
            options.name
        );
    }

    let (kind, source_path) = if let Some(builtin) = &options.builtin {
        let source_path = PathBuf::from(format!("./src/{}.cpp", options.name));
        let abs_source = resolve_path(&work_dir, &source_path);
        if abs_source.exists() && !options.replace {
            anyhow::bail!(
                "checker source {} already exists; omit --builtin to register the existing source, or pass --replace to overwrite it with the built-in checker",
                abs_source.display()
            );
        }
        let source = checker_source(builtin)?;
        if let Some(parent) = abs_source.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&abs_source, source)?;
        (AddProgramKind::Cpp, source_path)
    } else {
        resolve_program_path(&work_dir, &options.name, None, None)?
    };

    problem.programs.insert(
        options.name.clone(),
        program_from_parts(
            kind,
            source_path,
            options.time_limit_secs,
            options.memory_limit_mb,
            &options.compile_args,
            &problem.cpp_compile_args,
        ),
    );
    problem.checker_name = Some(options.name);
    write_problem(&work_dir, &problem)
}

fn builtin_checker_ids() -> impl Iterator<Item = &'static str> {
    generated_builtin_checkers::BUILTIN_CHECKERS
        .iter()
        .map(|(id, _, _)| *id)
}

fn builtin_checker(id: &str) -> Option<(&'static str, &'static str)> {
    generated_builtin_checkers::BUILTIN_CHECKERS
        .iter()
        .find(|(candidate, _, _)| *candidate == id)
        .map(|(_, filename, source)| (*filename, *source))
}

fn checker_source(builtin: &str) -> Result<String> {
    let Some((filename, source)) = builtin_checker(builtin) else {
        let available = builtin_checker_ids().collect::<Vec<_>>().join(", ");
        anyhow::bail!("unknown built-in checker `{builtin}`; available: {available}");
    };
    Ok(format!(
        "// Copied from testlib checkers/{filename}\n{source}"
    ))
}

fn program_from_parts(
    kind: AddProgramKind,
    path: PathBuf,
    time_limit_secs: Option<f64>,
    memory_limit_mb: Option<f64>,
    compile_args: &[String],
    default_cpp_compile_args: &[String],
) -> Program {
    let info = match kind {
        AddProgramKind::Cpp => ProgramInfo::Cpp(CppProgram {
            path,
            compile_args: if compile_args.is_empty() {
                default_cpp_compile_args.to_vec()
            } else {
                compile_args.to_vec()
            },
        }),
        AddProgramKind::Python => ProgramInfo::Python(CommandProgram {
            path,
            extra_args: Vec::new(),
        }),
        AddProgramKind::Command => ProgramInfo::Command(CommandProgram {
            path,
            extra_args: Vec::new(),
        }),
    };
    Program {
        info,
        time_limit_secs: time_limit_secs.unwrap_or(DEFAULT_ADD_TIME_LIMIT_SECS),
        memory_limit_mb: memory_limit_mb.unwrap_or(DEFAULT_ADD_MEMORY_LIMIT_MB),
    }
}

fn resolve_program_path(
    work_dir: &Path,
    name: &str,
    kind: Option<AddProgramKind>,
    explicit_path: Option<&Path>,
) -> Result<(AddProgramKind, PathBuf)> {
    if let Some(path) = explicit_path {
        let path = normalize_package_path(path);
        let abs_path = resolve_path(work_dir, &path);
        if !abs_path.is_file() {
            anyhow::bail!("source file {} does not exist", abs_path.display());
        }
        let kind = kind
            .or_else(|| infer_kind(&path))
            .unwrap_or(AddProgramKind::Command);
        return Ok((kind, path));
    }

    let candidates = [
        (
            AddProgramKind::Cpp,
            PathBuf::from(format!("./src/{name}.cpp")),
        ),
        (
            AddProgramKind::Python,
            PathBuf::from(format!("./src/{name}.py")),
        ),
        (
            AddProgramKind::Command,
            PathBuf::from(format!("./src/{name}")),
        ),
    ];
    let existing = candidates
        .iter()
        .filter(|(_, path)| resolve_path(work_dir, path).is_file())
        .cloned()
        .collect::<Vec<_>>();

    if let Some(kind) = kind {
        if let Some((_, path)) = existing
            .iter()
            .find(|(candidate_kind, _)| *candidate_kind == kind)
        {
            return Ok((kind, path.clone()));
        }
        let path = match kind {
            AddProgramKind::Cpp => PathBuf::from(format!("./src/{name}.cpp")),
            AddProgramKind::Python => PathBuf::from(format!("./src/{name}.py")),
            AddProgramKind::Command => PathBuf::from(format!("./src/{name}")),
        };
        anyhow::bail!(
            "source file {} does not exist",
            resolve_path(work_dir, &path).display()
        );
    }

    match existing.as_slice() {
        [] => anyhow::bail!(
            "no source file found for `{name}`; create src/{name}.cpp, src/{name}.py, or src/{name}, or pass --path to an existing source"
        ),
        [(kind, path)] => Ok((*kind, path.clone())),
        _ => anyhow::bail!(
            "multiple source candidates found for `{name}`; use `add program {name} --path ...` or `--kind ...` first, then register the role"
        ),
    }
}

fn infer_kind(path: &Path) -> Option<AddProgramKind> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("cpp" | "cc" | "cxx") => Some(AddProgramKind::Cpp),
        Some("py") => Some(AddProgramKind::Python),
        _ => None,
    }
}

fn normalize_package_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        let raw = path.to_string_lossy().replace('\\', "/");
        if raw.starts_with("./") {
            PathBuf::from(raw)
        } else {
            PathBuf::from(format!("./{raw}"))
        }
    }
}

fn read_problem(work_dir: &Path) -> Result<Problem> {
    let path = work_dir.join("problem.yaml");
    let yaml = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_yml::from_str(&yaml).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_problem(work_dir: &Path, problem: &Problem) -> Result<()> {
    std::fs::write(work_dir.join("problem.yaml"), render_problem(problem))?;
    Ok(())
}

fn normalize_work_dir(work_dir: &Path) -> Result<PathBuf> {
    if work_dir.is_absolute() {
        Ok(work_dir.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(work_dir))
    }
}

fn resolve_path(work_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        work_dir.join(path)
    }
}

fn render_problem(problem: &Problem) -> String {
    let mut out = String::new();
    out.push_str(&format!("name: {}\n", q(&problem.name)));
    out.push_str(&format!(
        "time_limit_secs: {}\n",
        format_f64(problem.time_limit_secs)
    ));
    out.push_str(&format!(
        "memory_limit_mb: {}\n",
        format_f64(problem.memory_limit_mb)
    ));
    out.push_str(&format!(
        "cpp_compile_args: {}\n",
        inline_list(&problem.cpp_compile_args)
    ));
    render_output(&mut out, &problem.output);
    render_programs(&mut out, problem);
    out.push_str(&format!(
        "solution: {}\n",
        key_or_string(&problem.solution_name)
    ));
    if let Some(validator) = &problem.validator_name {
        out.push_str(&format!("validator: {}\n", key_or_string(validator)));
    }
    if let Some(reason) = &problem.validator_omitted_reason {
        out.push_str(&format!("validator_omitted_reason: {}\n", q(reason)));
    }
    if let Some(checker) = &problem.checker_name {
        out.push_str(&format!("checker: {}\n", key_or_string(checker)));
    }
    if let Some(generator) = &problem.generator_name {
        out.push_str(&format!("generator: {}\n", key_or_string(generator)));
    }
    render_test(&mut out, problem);
    out
}

fn render_output(out: &mut String, output: &OutputConfig) {
    if output.allow_empty {
        out.push_str("output:\n  allow_empty: true\n");
    }
}

fn render_programs(out: &mut String, problem: &Problem) {
    out.push_str("programs:\n");
    let programs = &problem.programs;
    let mut names = programs.keys().collect::<Vec<_>>();
    names.sort();
    for name in names {
        let program = &programs[name];
        if let Some(path) = shorthand_program_path(problem, program) {
            out.push_str(&format!("  {}: {}\n", key_or_string(name), q_path(path)));
            continue;
        }
        out.push_str(&format!("  {}:\n", key_or_string(name)));
        match &program.info {
            ProgramInfo::Cpp(cpp) => {
                out.push_str("    info: !cpp\n");
                out.push_str(&format!("      path: {}\n", q_path(&cpp.path)));
                if cpp.compile_args != problem.cpp_compile_args {
                    out.push_str(&format!(
                        "      compile_args: {}\n",
                        inline_list(&cpp.compile_args)
                    ));
                }
            }
            ProgramInfo::Python(command) => {
                out.push_str("    info: !python\n");
                out.push_str(&format!("      path: {}\n", q_path(&command.path)));
                if !command.extra_args.is_empty() {
                    out.push_str(&format!(
                        "      extra_args: {}\n",
                        inline_list(&command.extra_args)
                    ));
                }
            }
            ProgramInfo::Command(command) => {
                out.push_str("    info: !command\n");
                out.push_str(&format!("      path: {}\n", q_path(&command.path)));
                if !command.extra_args.is_empty() {
                    out.push_str(&format!(
                        "      extra_args: {}\n",
                        inline_list(&command.extra_args)
                    ));
                }
            }
        }
        if program.time_limit_secs != problem.time_limit_secs {
            out.push_str(&format!(
                "    time_limit_secs: {}\n",
                format_f64(program.time_limit_secs)
            ));
        }
        if program.memory_limit_mb != problem.memory_limit_mb {
            out.push_str(&format!(
                "    memory_limit_mb: {}\n",
                format_f64(program.memory_limit_mb)
            ));
        }
    }
}

fn shorthand_program_path<'a>(problem: &Problem, program: &'a Program) -> Option<&'a Path> {
    if program.time_limit_secs != problem.time_limit_secs
        || program.memory_limit_mb != problem.memory_limit_mb
    {
        return None;
    }
    match &program.info {
        ProgramInfo::Cpp(cpp) if cpp.compile_args == problem.cpp_compile_args => {
            Some(cpp.path.as_path())
        }
        ProgramInfo::Python(command) if command.extra_args.is_empty() => {
            Some(command.path.as_path())
        }
        ProgramInfo::Command(_) | ProgramInfo::Cpp(_) | ProgramInfo::Python(_) => None,
    }
}

fn render_test(out: &mut String, problem: &Problem) {
    let test = &problem.test;
    out.push_str("test:\n  bundles:\n");
    let mut bundle_names = test.bundles.keys().collect::<Vec<_>>();
    bundle_names.sort();
    for name in bundle_names {
        out.push_str(&format!("    {}:\n", key_or_string(name)));
        let cases = &test.bundles[name].cases;
        let all_default_generator = cases
            .iter()
            .all(|case| problem.generator_name.as_deref() == Some(case.generator_name.as_str()));
        if !cases.is_empty() && all_default_generator {
            for case in cases {
                out.push_str(&format!("      - {}\n", inline_list(&case.args)));
            }
            continue;
        }
        out.push_str("      cases:\n");
        for case in cases {
            if problem.generator_name.as_deref() == Some(case.generator_name.as_str()) {
                out.push_str(&format!("      - {}\n", inline_list(&case.args)));
            } else {
                out.push_str(&format!(
                    "      - generator: {}\n",
                    key_or_string(&case.generator_name)
                ));
                out.push_str(&format!("        args: {}\n", inline_list(&case.args)));
            }
        }
    }
    out.push_str("  tasks:\n");
    for task in &test.tasks {
        out.push_str(&format!("  - name: {}\n", key_or_string(&task.name)));
        if let Some(score) = task.score {
            out.push_str(&format!("    score: {}\n", format_f64(score)));
        }
        if let Some(task_type) = task.task_type {
            out.push_str(&format!("    type: {}\n", task_type_name(task_type)));
        }
        if !task.bundles.is_empty() {
            out.push_str(&format!("    bundles: {}\n", inline_list(&task.bundles)));
        }
        if !task.cases.is_empty() {
            out.push_str("    cases:\n");
            for case in &task.cases {
                if problem.generator_name.as_deref() == Some(case.generator_name.as_str()) {
                    out.push_str(&format!("    - {}\n", inline_list(&case.args)));
                } else {
                    out.push_str(&format!(
                        "    - generator: {}\n",
                        key_or_string(&case.generator_name)
                    ));
                    out.push_str(&format!("      args: {}\n", inline_list(&case.args)));
                }
            }
        }
        if !task.dependencies.is_empty() {
            out.push_str(&format!(
                "    dependencies: {}\n",
                inline_list(&task.dependencies)
            ));
        }
        if !task.pass_programs.is_empty() {
            out.push_str(&format!("    pass: {}\n", inline_list(&task.pass_programs)));
        }
        if !task.fail_programs.is_empty() {
            out.push_str(&format!("    fail: {}\n", inline_list(&task.fail_programs)));
        }
    }
}

fn task_type_name(task_type: TestTaskType) -> &'static str {
    match task_type {
        TestTaskType::Sum => "sum",
        TestTaskType::Min => "min",
    }
}

fn key_or_string(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        && !value.is_empty()
    {
        value.to_string()
    } else {
        q(value)
    }
}

fn q_path(path: &Path) -> String {
    q(&path.to_string_lossy().replace('\\', "/"))
}

fn q(value: &str) -> String {
    serde_json::to_string(value).expect("serializing string cannot fail")
}

fn inline_list(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| q(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn format_f64(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::{init_package, load_problem, temp_test_dir};

    fn remove_default_checker(problem_dir: &Path) {
        let mut problem = read_problem(problem_dir).unwrap();
        problem.checker_name = None;
        problem.programs.remove("chk");
        write_problem(problem_dir, &problem).unwrap();
        let source_path = problem_dir.join("src").join("chk.cpp");
        if source_path.exists() {
            std::fs::remove_file(source_path).unwrap();
        }
    }

    fn clear_default_checker_role(problem_dir: &Path) {
        let mut problem = read_problem(problem_dir).unwrap();
        problem.checker_name = None;
        write_problem(problem_dir, &problem).unwrap();
    }

    #[test]
    fn add_program_requires_existing_source_and_registers_cpp() {
        let root = temp_test_dir("cptool-add-program");
        let problem_dir = init_package(&root, "Add Program").unwrap();
        let source_path = problem_dir.join("src").join("foo.cpp");
        std::fs::write(
            &source_path,
            "#include <bits/stdc++.h>\nint main(){return 0;}\n",
        )
        .unwrap();

        add_program(AddProgramOptions {
            work_dir: problem_dir.clone(),
            name: "foo".to_string(),
            kind: None,
            path: None,
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        let problem = load_problem(&problem_dir).unwrap();
        let ProgramInfo::Cpp(cpp) = &problem.programs["foo"].info else {
            panic!("expected C++ program");
        };
        assert_eq!(cpp.path, PathBuf::from("./src/foo.cpp"));
        assert_eq!(cpp.compile_args, ["-O2", "-std=c++20"]);
        assert_eq!(problem.programs["foo"].time_limit_secs, 3.0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_program_rejects_missing_source() {
        let root = temp_test_dir("cptool-add-program-missing-source");
        let problem_dir = init_package(&root, "Add Program Missing Source").unwrap();

        let err = add_program(AddProgramOptions {
            work_dir: problem_dir.clone(),
            name: "foo".to_string(),
            kind: None,
            path: None,
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap_err()
        .to_string();

        assert!(err.contains("no source file found for `foo`"));
        assert!(!problem_dir.join("src").join("foo.cpp").exists());

        let err = add_program(AddProgramOptions {
            work_dir: problem_dir.clone(),
            name: "bar".to_string(),
            kind: Some(AddProgramKind::Cpp),
            path: None,
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("source file"));
        assert!(err.contains("bar.cpp"));

        let err = add_program(AddProgramOptions {
            work_dir: problem_dir.clone(),
            name: "baz".to_string(),
            kind: None,
            path: Some(PathBuf::from("./src/missing.py")),
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("source file"));
        assert!(err.contains("missing.py"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_program_detects_existing_python_and_custom_limits() {
        let root = temp_test_dir("cptool-add-python");
        let problem_dir = init_package(&root, "Add Python").unwrap();
        std::fs::write(problem_dir.join("src").join("foo.py"), "print(1)\n").unwrap();

        add_program(AddProgramOptions {
            work_dir: problem_dir.clone(),
            name: "foo".to_string(),
            kind: None,
            path: None,
            time_limit_secs: Some(2.5),
            memory_limit_mb: Some(64.0),
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        let problem = load_problem(&problem_dir).unwrap();
        assert!(matches!(
            problem.programs["foo"].info,
            ProgramInfo::Python(_)
        ));
        assert_eq!(problem.programs["foo"].time_limit_secs, 2.5);
        assert_eq!(problem.programs["foo"].memory_limit_mb, 64.0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_bundle_task_and_replace_work() {
        let root = temp_test_dir("cptool-add-bundle-task");
        let problem_dir = init_package(&root, "Add Bundle").unwrap();

        add_bundle(AddBundleOptions {
            work_dir: problem_dir.clone(),
            name: "extra".to_string(),
            generator: Some("gen".to_string()),
            cases: vec![
                vec!["a".to_string()],
                vec!["b".to_string(), "c".to_string()],
            ],
            replace: false,
        })
        .unwrap();
        assert!(
            add_bundle(AddBundleOptions {
                work_dir: problem_dir.clone(),
                name: "extra".to_string(),
                generator: Some("gen".to_string()),
                cases: vec![vec!["z".to_string()]],
                replace: false,
            })
            .is_err()
        );
        add_task(AddTaskOptions {
            work_dir: problem_dir.clone(),
            name: "extra_task".to_string(),
            score: Some(50.0),
            task_type: TestTaskType::Sum,
            bundles: vec!["extra".to_string()],
            dependencies: vec!["sample".to_string()],
            expect_pass: Vec::new(),
            expect_fail: Vec::new(),
            replace: false,
        })
        .unwrap();

        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.test.bundles["extra"].cases.len(), 2);
        assert!(
            problem
                .test
                .tasks
                .iter()
                .any(|task| task.name == "extra_task" && task.task_type == Some(TestTaskType::Sum))
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_checker_copies_builtin_with_origin_comment() {
        let root = temp_test_dir("cptool-add-checker");
        let problem_dir = init_package(&root, "Add Checker").unwrap();
        remove_default_checker(&problem_dir);

        add_checker(AddCheckerOptions {
            work_dir: problem_dir.clone(),
            name: "chk".to_string(),
            builtin: Some("wcmp".to_string()),
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        let source = std::fs::read_to_string(problem_dir.join("src").join("chk.cpp")).unwrap();
        assert!(source.starts_with("// Copied from testlib checkers/wcmp.cpp\n"));
        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.checker_name.as_deref(), Some("chk"));
        assert!(problem.programs.contains_key("chk"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_validator_registers_existing_custom_source_without_replacing_it() {
        let root = temp_test_dir("cptool-add-validator-existing-custom");
        let problem_dir = init_package(&root, "Add Validator Existing Custom").unwrap();
        let mut problem = read_problem(&problem_dir).unwrap();
        problem.validator_name = None;
        problem.programs.remove("val");
        write_problem(&problem_dir, &problem).unwrap();
        let custom_source =
            "#include \"testlib.h\"\nint main() { registerValidation(); inf.readEof(); }\n";
        std::fs::write(problem_dir.join("src").join("val.cpp"), custom_source).unwrap();

        add_validator(AddValidatorOptions {
            work_dir: problem_dir.clone(),
            name: "val".to_string(),
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.validator_name.as_deref(), Some("val"));
        assert!(matches!(problem.programs["val"].info, ProgramInfo::Cpp(_)));
        let source = std::fs::read_to_string(problem_dir.join("src").join("val.cpp")).unwrap();
        assert_eq!(source, custom_source);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_validator_rejects_missing_source() {
        let root = temp_test_dir("cptool-add-validator-create");
        let problem_dir = init_package(&root, "Add Validator Create").unwrap();
        let mut problem = read_problem(&problem_dir).unwrap();
        problem.validator_name = None;
        problem.programs.remove("val");
        write_problem(&problem_dir, &problem).unwrap();
        std::fs::remove_file(problem_dir.join("src").join("val.cpp")).unwrap();

        let err = add_validator(AddValidatorOptions {
            work_dir: problem_dir.clone(),
            name: "val".to_string(),
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap_err()
        .to_string();

        let source_path = problem_dir.join("src").join("val.cpp");
        assert!(err.contains("no source file found for `val`"));
        assert!(!source_path.exists());
        let problem = load_problem(&problem_dir).unwrap();
        assert!(problem.validator_name.is_none());
        assert!(!problem.programs.contains_key("val"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_validator_can_mark_existing_program_and_clears_omitted_reason() {
        let root = temp_test_dir("cptool-add-validator-existing-program");
        let problem_dir = init_package(&root, "Add Validator Existing Program").unwrap();
        let mut problem = read_problem(&problem_dir).unwrap();
        problem.validator_name = None;
        problem.validator_omitted_reason = Some("temporary fixture".to_string());
        write_problem(&problem_dir, &problem).unwrap();

        add_validator(AddValidatorOptions {
            work_dir: problem_dir.clone(),
            name: "val".to_string(),
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.validator_name.as_deref(), Some("val"));
        assert!(problem.validator_omitted_reason.is_none());
        let yaml = std::fs::read_to_string(problem_dir.join("problem.yaml")).unwrap();
        assert!(!yaml.contains("validator_omitted_reason"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_validator_requires_replace_when_already_configured() {
        let root = temp_test_dir("cptool-add-validator-replace");
        let problem_dir = init_package(&root, "Add Validator Replace").unwrap();

        let err = add_validator(AddValidatorOptions {
            work_dir: problem_dir.clone(),
            name: "val".to_string(),
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("validator is already configured"));

        add_validator(AddValidatorOptions {
            work_dir: problem_dir.clone(),
            name: "val".to_string(),
            time_limit_secs: Some(2.0),
            memory_limit_mb: Some(256.0),
            compile_args: vec!["-O2".to_string()],
            replace: true,
        })
        .unwrap();
        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.validator_name.as_deref(), Some("val"));
        assert_eq!(problem.programs["val"].time_limit_secs, 2.0);
        assert_eq!(problem.programs["val"].memory_limit_mb, 256.0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_checker_registers_existing_custom_source_without_builtin() {
        let root = temp_test_dir("cptool-add-checker-existing-custom");
        let problem_dir = init_package(&root, "Add Checker Existing Custom").unwrap();
        clear_default_checker_role(&problem_dir);
        let custom_source = "#include \"testlib.h\"\nint main(int argc, char** argv) { registerTestlibCmd(argc, argv); quitf(_ok, \"ok\"); }\n";
        std::fs::write(problem_dir.join("src").join("chk.cpp"), custom_source).unwrap();

        add_checker(AddCheckerOptions {
            work_dir: problem_dir.clone(),
            name: "chk".to_string(),
            builtin: None,
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.checker_name.as_deref(), Some("chk"));
        assert!(matches!(problem.programs["chk"].info, ProgramInfo::Cpp(_)));
        let source = std::fs::read_to_string(problem_dir.join("src").join("chk.cpp")).unwrap();
        assert_eq!(source, custom_source);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_checker_rejects_missing_source_without_builtin() {
        let root = temp_test_dir("cptool-add-checker-create");
        let problem_dir = init_package(&root, "Add Checker Create").unwrap();
        remove_default_checker(&problem_dir);

        let err = add_checker(AddCheckerOptions {
            work_dir: problem_dir.clone(),
            name: "chk".to_string(),
            builtin: None,
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap_err()
        .to_string();

        let source_path = problem_dir.join("src").join("chk.cpp");
        assert!(err.contains("no source file found for `chk`"));
        assert!(!source_path.exists());
        let problem = load_problem(&problem_dir).unwrap();
        assert!(problem.checker_name.is_none());
        assert!(!problem.programs.contains_key("chk"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn add_checker_can_mark_existing_program_as_checker() {
        let root = temp_test_dir("cptool-add-checker-existing-program");
        let problem_dir = init_package(&root, "Add Checker Existing Program").unwrap();
        remove_default_checker(&problem_dir);
        std::fs::write(
            problem_dir.join("src").join("chk.cpp"),
            "#include \"testlib.h\"\nint main(int argc, char** argv) { registerTestlibCmd(argc, argv); quitf(_ok, \"ok\"); }\n",
        )
        .unwrap();
        add_program(AddProgramOptions {
            work_dir: problem_dir.clone(),
            name: "chk".to_string(),
            kind: Some(AddProgramKind::Cpp),
            path: None,
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        add_checker(AddCheckerOptions {
            work_dir: problem_dir.clone(),
            name: "chk".to_string(),
            builtin: None,
            time_limit_secs: None,
            memory_limit_mb: None,
            compile_args: Vec::new(),
            replace: false,
        })
        .unwrap();

        let problem = load_problem(&problem_dir).unwrap();
        assert_eq!(problem.checker_name.as_deref(), Some("chk"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_builtin_checker_registry_includes_directory_entries() {
        let ids = builtin_checker_ids().collect::<Vec<_>>();
        assert!(ids.contains(&"wcmp"));
        assert!(ids.contains(&"yesno"));
        assert!(ids.contains(&"pointscmp"));
    }
}
