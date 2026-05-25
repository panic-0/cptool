use super::problem::{FILE_GENERATOR_NAME, load_problem_read_only, normalize_work_dir};
use super::schema::{Problem, Program, ProgramInfo, TestCase};
use anyhow::Result;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ExplainOptions {
    pub work_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainReport {
    pub work_dir: PathBuf,
    pub roles: ExplainRoles,
    pub programs: Vec<ExplainProgram>,
    pub official_data: ExplainOfficialData,
    pub expect_checks: Vec<ExplainExpectTask>,
    pub handwritten_inputs: Vec<ExplainHandwrittenInput>,
}

impl ExplainReport {
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# cptool pkg explain\n\n");
        out.push_str("roles\n");
        render_role(&mut out, "solution", self.roles.solution.as_ref());
        render_role(&mut out, "validator", self.roles.validator.as_ref());
        render_role(&mut out, "checker", self.roles.checker.as_ref());
        render_role(&mut out, "generator", self.roles.generator.as_ref());

        out.push_str("\nprograms\n");
        if self.programs.is_empty() {
            out.push_str("  none\n");
        } else {
            for program in &self.programs {
                out.push_str(&format!(
                    "  {}: {} {}{}\n",
                    program.name,
                    program.kind,
                    program.path,
                    render_program_roles(&program.roles)
                ));
            }
        }

        out.push_str("\nofficial data\n");
        if self.official_data.tasks.is_empty() {
            out.push_str("  tasks: none\n");
        } else {
            for task in &self.official_data.tasks {
                out.push_str(&format!(
                    "  task {} score={}{} bundles={} pass={} fail={} generators={}\n",
                    task.name,
                    render_score(task.score),
                    render_task_type(task.task_type.as_deref()),
                    render_list(&task.bundles),
                    render_list(&task.pass),
                    render_list(&task.fail),
                    render_list(&task.generators),
                ));
            }
        }
        if self.official_data.bundles.is_empty() {
            out.push_str("  bundles: none\n");
        } else {
            for bundle in &self.official_data.bundles {
                out.push_str(&format!(
                    "  bundle {} cases={} generators={}\n",
                    bundle.name,
                    bundle.cases,
                    render_list(&bundle.generators)
                ));
            }
        }

        out.push_str("\nexpect checks\n");
        if self.expect_checks.is_empty() {
            out.push_str("  none\n");
        } else {
            for task in &self.expect_checks {
                let official = if task.official {
                    format!("score={}", render_score(task.score.unwrap_or_default()))
                } else {
                    "verify-only".to_string()
                };
                out.push_str(&format!(
                    "  task {} {} bundles={} inline_cases={} pass={} fail={} generators={}\n",
                    task.name,
                    official,
                    render_list(&task.bundles),
                    task.inline_cases,
                    render_list(&task.pass),
                    render_list(&task.fail),
                    render_list(&task.generators),
                ));
            }
        }

        out.push_str("\nhandwritten inputs\n");
        if self.handwritten_inputs.is_empty() {
            out.push_str("  none\n");
        } else {
            for input in &self.handwritten_inputs {
                out.push_str(&format!(
                    "  {} used_by={}\n",
                    input.path,
                    render_list(&input.used_by)
                ));
            }
        }
        out
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainRoles {
    pub solution: Option<ExplainProgramRef>,
    pub validator: Option<ExplainProgramRef>,
    pub checker: Option<ExplainProgramRef>,
    pub generator: Option<ExplainProgramRef>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainProgramRef {
    pub name: String,
    pub kind: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainProgram {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub roles: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainOfficialData {
    pub bundles: Vec<ExplainBundle>,
    pub tasks: Vec<ExplainOfficialTask>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainBundle {
    pub name: String,
    pub cases: usize,
    pub generators: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainOfficialTask {
    pub name: String,
    pub score: f64,
    #[serde(rename = "type")]
    pub task_type: Option<String>,
    pub bundles: Vec<String>,
    pub pass: Vec<String>,
    pub fail: Vec<String>,
    pub generators: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainExpectTask {
    pub name: String,
    pub official: bool,
    pub score: Option<f64>,
    #[serde(rename = "type")]
    pub task_type: Option<String>,
    pub bundles: Vec<String>,
    pub inline_cases: usize,
    pub pass: Vec<String>,
    pub fail: Vec<String>,
    pub generators: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExplainHandwrittenInput {
    pub path: String,
    pub used_by: Vec<String>,
}

pub fn explain_package(options: ExplainOptions) -> Result<ExplainReport> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let problem = load_problem_read_only(&work_dir)?;
    Ok(explain_problem(&work_dir, &problem))
}

fn explain_problem(work_dir: &Path, problem: &Problem) -> ExplainReport {
    let role_names = role_names(problem);
    let roles = ExplainRoles {
        solution: program_ref(work_dir, &problem.solution_name, problem),
        validator: problem
            .validator_name
            .as_ref()
            .and_then(|name| program_ref(work_dir, name, problem)),
        checker: problem
            .checker_name
            .as_ref()
            .and_then(|name| program_ref(work_dir, name, problem)),
        generator: problem
            .generator_name
            .as_ref()
            .map(|name| generator_ref(work_dir, name, problem)),
    };

    let mut programs = problem
        .programs
        .iter()
        .filter(|(name, _)| !role_names.contains_key(name.as_str()))
        .map(|(name, program)| {
            let (kind, path) = program_kind_path(program);
            ExplainProgram {
                name: name.clone(),
                kind: kind.to_string(),
                path: display_path(work_dir, path),
                roles: program_usage_roles(name, problem),
            }
        })
        .collect::<Vec<_>>();
    programs.sort_by(|left, right| left.name.cmp(&right.name));

    let official_bundle_names = official_bundle_names(problem);
    let mut bundles = official_bundle_names
        .iter()
        .filter_map(|name| {
            problem.test.bundles.get(name).map(|bundle| ExplainBundle {
                name: name.clone(),
                cases: bundle.cases.len(),
                generators: case_generators(&bundle.cases),
            })
        })
        .collect::<Vec<_>>();
    bundles.sort_by(|left, right| left.name.cmp(&right.name));

    let tasks = problem
        .test
        .tasks
        .iter()
        .filter(|task| task.is_official())
        .map(|task| ExplainOfficialTask {
            name: task.name.clone(),
            score: task.score.unwrap_or_default(),
            task_type: task.task_type.map(task_type_name),
            bundles: task.bundles.clone(),
            pass: task.pass_programs.clone(),
            fail: task.fail_programs.clone(),
            generators: task_generators(problem, &task.bundles, &task.cases),
        })
        .collect();

    let expect_checks = problem
        .test
        .tasks
        .iter()
        .filter(|task| task.has_expectations())
        .map(|task| ExplainExpectTask {
            name: task.name.clone(),
            official: task.is_official(),
            score: task.score,
            task_type: task.task_type.map(task_type_name),
            bundles: task.bundles.clone(),
            inline_cases: task.cases.len(),
            pass: task.pass_programs.clone(),
            fail: task.fail_programs.clone(),
            generators: task_generators(problem, &task.bundles, &task.cases),
        })
        .collect();

    ExplainReport {
        work_dir: work_dir.to_path_buf(),
        roles,
        programs,
        official_data: ExplainOfficialData { bundles, tasks },
        expect_checks,
        handwritten_inputs: handwritten_inputs(problem),
    }
}

fn role_names(problem: &Problem) -> HashMap<&str, Vec<&'static str>> {
    let mut roles: HashMap<&str, Vec<&'static str>> = HashMap::new();
    roles
        .entry(problem.solution_name.as_str())
        .or_default()
        .push("solution");
    if let Some(name) = &problem.validator_name {
        roles.entry(name.as_str()).or_default().push("validator");
    }
    if let Some(name) = &problem.checker_name {
        roles.entry(name.as_str()).or_default().push("checker");
    }
    if let Some(name) = &problem.generator_name
        && problem.programs.contains_key(name)
    {
        roles.entry(name.as_str()).or_default().push("generator");
    }
    roles
}

fn program_ref(work_dir: &Path, name: &str, problem: &Problem) -> Option<ExplainProgramRef> {
    problem.programs.get(name).map(|program| {
        let (kind, path) = program_kind_path(program);
        ExplainProgramRef {
            name: name.to_string(),
            kind: kind.to_string(),
            path: display_path(work_dir, path),
        }
    })
}

fn generator_ref(work_dir: &Path, name: &str, problem: &Problem) -> ExplainProgramRef {
    if let Some(program) = problem.programs.get(name) {
        let (kind, path) = program_kind_path(program);
        ExplainProgramRef {
            name: name.to_string(),
            kind: kind.to_string(),
            path: display_path(work_dir, path),
        }
    } else {
        ExplainProgramRef {
            name: name.to_string(),
            kind: "builtin".to_string(),
            path: name.to_string(),
        }
    }
}

fn program_kind_path(program: &Program) -> (&'static str, &Path) {
    match &program.info {
        ProgramInfo::Cpp(cpp) => ("cpp", &cpp.path),
        ProgramInfo::Python(command) => ("python", &command.path),
        ProgramInfo::Command(command) => ("command", &command.path),
    }
}

fn program_usage_roles(name: &str, problem: &Problem) -> Vec<String> {
    let mut roles = BTreeSet::new();
    if problem
        .test
        .bundles
        .values()
        .any(|bundle| bundle.cases.iter().any(|case| case.generator_name == name))
        || problem
            .test
            .tasks
            .iter()
            .any(|task| task.cases.iter().any(|case| case.generator_name == name))
    {
        roles.insert("case_generator".to_string());
    }
    if problem.test.tasks.iter().any(|task| {
        task.pass_programs.iter().any(|program| program == name)
            || task.fail_programs.iter().any(|program| program == name)
    }) {
        roles.insert("expect_program".to_string());
    }
    roles.into_iter().collect()
}

fn official_bundle_names(problem: &Problem) -> BTreeSet<String> {
    problem
        .test
        .tasks
        .iter()
        .filter(|task| task.is_official())
        .flat_map(|task| task.bundles.iter().cloned())
        .collect()
}

fn task_generators(
    problem: &Problem,
    bundles: &[String],
    inline_cases: &[TestCase],
) -> Vec<String> {
    let mut generators = BTreeSet::new();
    for bundle_name in bundles {
        if let Some(bundle) = problem.test.bundles.get(bundle_name) {
            generators.extend(bundle.cases.iter().map(|case| case.generator_name.clone()));
        }
    }
    generators.extend(inline_cases.iter().map(|case| case.generator_name.clone()));
    generators.into_iter().collect()
}

fn case_generators(cases: &[TestCase]) -> Vec<String> {
    cases
        .iter()
        .map(|case| case.generator_name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn handwritten_inputs(problem: &Problem) -> Vec<ExplainHandwrittenInput> {
    let mut used: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (bundle_name, bundle) in &problem.test.bundles {
        for (index, case) in bundle.cases.iter().enumerate() {
            if case.generator_name == FILE_GENERATOR_NAME
                && let Some(path) = case.args.first()
            {
                used.entry(path.clone())
                    .or_default()
                    .push(format!("{bundle_name}[{index}]"));
            }
        }
    }
    for task in &problem.test.tasks {
        for (index, case) in task.cases.iter().enumerate() {
            if case.generator_name == FILE_GENERATOR_NAME
                && let Some(path) = case.args.first()
            {
                used.entry(path.clone())
                    .or_default()
                    .push(format!("task:{}[{index}]", task.name));
            }
        }
    }
    used.into_iter()
        .map(|(path, used_by)| ExplainHandwrittenInput { path, used_by })
        .collect()
}

fn task_type_name(task_type: super::schema::TestTaskType) -> String {
    match task_type {
        super::schema::TestTaskType::Sum => "sum".to_string(),
        super::schema::TestTaskType::Min => "min".to_string(),
    }
}

fn display_path(work_dir: &Path, path: &Path) -> String {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        work_dir.join(path)
    };
    joined
        .strip_prefix(work_dir)
        .unwrap_or(joined.as_path())
        .to_string_lossy()
        .replace('\\', "/")
}

fn render_role(out: &mut String, label: &str, role: Option<&ExplainProgramRef>) {
    if let Some(role) = role {
        out.push_str(&format!(
            "  {label}: {} -> {} ({})\n",
            role.name, role.path, role.kind
        ));
    } else {
        out.push_str(&format!("  {label}: none\n"));
    }
}

fn render_program_roles(roles: &[String]) -> String {
    if roles.is_empty() {
        String::new()
    } else {
        format!(" roles={}", render_list(roles))
    }
}

fn render_list(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", values.join(","))
    }
}

fn render_score(score: f64) -> String {
    if score.fract() == 0.0 {
        format!("{score:.1}")
    } else {
        score.to_string()
    }
}

fn render_task_type(task_type: Option<&str>) -> String {
    task_type
        .map(|value| format!(" type={value}"))
        .unwrap_or_default()
}
