use super::{CheckReport, codes};
use serde_yml::{Mapping, Value};
use std::path::Path;

pub(super) fn check_unknown_yaml_fields(report: &mut CheckReport, work_dir: &Path) {
    let path = work_dir.join("problem.yaml");
    let Ok(yaml) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(value) = serde_yml::from_str::<Value>(&yaml) else {
        return;
    };
    let Some(root) = value_mapping(&value) else {
        return;
    };

    warn_unknown_keys(
        report,
        &path,
        root,
        "",
        &[
            "name",
            "time_limit_secs",
            "memory_limit_mb",
            "cpp_compile_args",
            "output",
            "stress",
            "programs",
            "test",
            "solution",
            "validator",
            "validator_omitted_reason",
            "checker",
            "generator",
        ],
    );

    if let Some(output) = mapping_get(root, "output").and_then(value_mapping) {
        warn_unknown_keys(report, &path, output, "output", &["allow_empty"]);
    }
    if let Some(programs) = mapping_get(root, "programs").and_then(value_mapping) {
        for (program_name, program_value) in string_entries(programs) {
            let program_location = format!("programs.{program_name}");
            let Some(program) = value_mapping(program_value) else {
                continue;
            };
            warn_unknown_keys(
                report,
                &path,
                program,
                &program_location,
                &["info", "time_limit_secs", "memory_limit_mb"],
            );
            if let Some(info) = mapping_get(program, "info").and_then(value_mapping) {
                warn_unknown_keys(
                    report,
                    &path,
                    info,
                    &format!("{program_location}.info"),
                    &["path", "compile_args", "extra_args"],
                );
            }
        }
    }
    if let Some(test) = mapping_get(root, "test").and_then(value_mapping) {
        warn_unknown_keys(
            report,
            &path,
            test,
            "test",
            &["generator", "type", "bundles", "tasks"],
        );
        if let Some(bundles) = mapping_get(test, "bundles").and_then(value_mapping) {
            for (bundle_name, bundle_value) in string_entries(bundles) {
                let bundle_location = format!("test.bundles.{bundle_name}");
                let Some(bundle) = value_mapping(bundle_value) else {
                    continue;
                };
                warn_unknown_keys(
                    report,
                    &path,
                    bundle,
                    &bundle_location,
                    &["generator", "cases"],
                );
                if let Some(cases) = mapping_get(bundle, "cases").and_then(value_sequence) {
                    for (case_index, case_value) in cases.iter().enumerate() {
                        if let Some(case) = value_mapping(case_value) {
                            warn_unknown_keys(
                                report,
                                &path,
                                case,
                                &format!("{bundle_location}.cases[{case_index}]"),
                                &["generator", "args"],
                            );
                        }
                    }
                }
            }
        }
        if let Some(tasks) = mapping_get(test, "tasks").and_then(value_sequence) {
            for (task_index, task_value) in tasks.iter().enumerate() {
                if let Some(task) = value_mapping(task_value) {
                    warn_unknown_keys(
                        report,
                        &path,
                        task,
                        &format!("test.tasks[{task_index}]"),
                        &["name", "score", "type", "bundles", "dependencies"],
                    );
                }
            }
        }
    }
    if let Some(stress) = mapping_get(root, "stress").and_then(value_mapping) {
        warn_unknown_keys(report, &path, stress, "stress", &["plans"]);
        if let Some(plans) = mapping_get(stress, "plans").and_then(value_sequence) {
            for (plan_index, plan_value) in plans.iter().enumerate() {
                if let Some(plan) = value_mapping(plan_value) {
                    warn_unknown_keys(
                        report,
                        &path,
                        plan,
                        &format!("stress.plans[{plan_index}]"),
                        &["name", "generator", "args", "against", "cases", "expect"],
                    );
                }
            }
        }
    }
}

fn value_mapping(value: &Value) -> Option<&Mapping> {
    match value {
        Value::Mapping(mapping) => Some(mapping),
        Value::Tagged(tagged) => value_mapping(&tagged.value),
        _ => None,
    }
}

fn value_sequence(value: &Value) -> Option<&[Value]> {
    match value {
        Value::Sequence(sequence) => Some(sequence),
        Value::Tagged(tagged) => value_sequence(&tagged.value),
        _ => None,
    }
}

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping
        .map
        .iter()
        .find_map(|(candidate, value)| match candidate {
            Value::String(candidate) if candidate == key => Some(value),
            _ => None,
        })
}

fn string_entries(mapping: &Mapping) -> impl Iterator<Item = (&str, &Value)> {
    mapping.map.iter().filter_map(|(key, value)| match key {
        Value::String(key) => Some((key.as_str(), value)),
        _ => None,
    })
}

fn warn_unknown_keys(
    report: &mut CheckReport,
    path: &Path,
    mapping: &Mapping,
    location: &str,
    allowed: &[&str],
) {
    for (key, _value) in &mapping.map {
        let key = match key {
            Value::String(key) => key,
            _ => {
                report.warning_at(
                    codes::UNKNOWN_FIELD,
                    "non-string YAML key is ignored by cptool",
                    Some(path.to_path_buf()),
                    if location.is_empty() {
                        "<root>"
                    } else {
                        location
                    },
                );
                continue;
            }
        };
        if !allowed.contains(&key.as_str()) {
            let field_location = if location.is_empty() {
                key.to_string()
            } else {
                format!("{location}.{key}")
            };
            report.warning_at(
                codes::UNKNOWN_FIELD,
                format!("unknown problem.yaml field `{key}`"),
                Some(path.to_path_buf()),
                field_location,
            );
        }
    }
}
