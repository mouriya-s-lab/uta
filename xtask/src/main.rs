//! Repository checks for workspace package boundaries.

use std::{
    collections::BTreeSet,
    env,
    path::PathBuf,
    process::{Command, ExitCode},
};

use serde::Deserialize;

const ALLOW_LIST: &[(&str, &[&str])] = &[
    ("uta-base", &[]),
    ("uta-proc", &[]),
    ("uta-rpc", &[]),
    ("uta-channel", &[]),
    ("uta-store", &["uta-base", "uta-proc"]),
    ("uta-core", &["uta-base", "uta-proc", "uta-store"]),
    (
        "uta-testkit",
        &[
            "uta-base",
            "uta-proc",
            "uta-rpc",
            "uta-channel",
            "uta-store",
        ],
    ),
    ("xtask", &[]),
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    match (args.next(), args.next()) {
        (Some(command), None) if command == "check-deps" => check_dependencies(),
        _ => Err("usage: cargo xtask check-deps".to_owned()),
    }
}

fn check_dependencies() -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|error| format!("failed to run `cargo metadata`: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`cargo metadata --format-version 1 --no-deps` failed with {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse `cargo metadata` output: {error}"))?;
    let workspace_member_ids: BTreeSet<_> = metadata.workspace_members.into_iter().collect();

    let mut packages = Vec::with_capacity(workspace_member_ids.len());
    let mut seen_ids = BTreeSet::new();
    let mut seen_names = BTreeSet::new();
    for package in metadata.packages {
        if !workspace_member_ids.contains(&package.id) {
            continue;
        }

        if !seen_ids.insert(package.id.clone()) {
            return Err(format!(
                "invalid cargo metadata: duplicate workspace package id `{}`",
                package.id
            ));
        }
        if !seen_names.insert(package.name.clone()) {
            return Err(format!(
                "invalid cargo metadata: duplicate workspace package name `{}`",
                package.name
            ));
        }
        packages.push(package);
    }

    for member_id in &workspace_member_ids {
        if !seen_ids.contains(member_id) {
            return Err(format!(
                "invalid cargo metadata: workspace member `{member_id}` has no package entry"
            ));
        }
    }

    let mut missing_allow_list = Vec::new();
    let mut forbidden_edges = BTreeSet::new();

    for package in &packages {
        let allowed_dependencies = allowed_dependencies(&package.name);
        if allowed_dependencies.is_none() {
            missing_allow_list.push(package.name.clone());
        }

        for dependency in &package.dependencies {
            if matches!(dependency.kind, Some(DependencyKind::Dev)) {
                continue;
            }

            if let Some(target) = workspace_dependency_target(dependency, &packages)?
                && target.name != package.name
                && !allowed_dependencies
                    .is_some_and(|allowed| allowed.contains(&target.name.as_str()))
            {
                forbidden_edges.insert((package.name.clone(), target.name.clone()));
            }
        }
    }

    missing_allow_list.sort_unstable();
    if !missing_allow_list.is_empty() || !forbidden_edges.is_empty() {
        let mut lines = vec!["dependency policy violations:".to_owned()];
        for package in missing_allow_list {
            lines.push(format!(
                "  workspace package `{package}` is missing from the allow-list"
            ));
        }
        for (from, to) in forbidden_edges {
            lines.push(format!(
                "  forbidden workspace dependency edge: {from} -> {to}"
            ));
        }
        return Err(lines.join("\n"));
    }

    println!(
        "Dependency policy OK: checked {} workspace packages.",
        packages.len()
    );
    Ok(())
}

#[derive(Deserialize)]
struct CargoMetadata {
    workspace_members: Vec<String>,
    packages: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    source: Option<String>,
    manifest_path: PathBuf,
    dependencies: Vec<CargoDependency>,
}

#[derive(Deserialize)]
struct CargoDependency {
    name: String,
    source: Option<String>,
    kind: Option<DependencyKind>,
    path: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum DependencyKind {
    Build,
    Dev,
}

fn allowed_dependencies(package: &str) -> Option<&'static [&'static str]> {
    ALLOW_LIST
        .iter()
        .find_map(|(name, dependencies)| (*name == package).then_some(*dependencies))
}

fn workspace_dependency_target<'a>(
    dependency: &CargoDependency,
    packages: &'a [CargoPackage],
) -> Result<Option<&'a CargoPackage>, String> {
    let Some(package) = packages
        .iter()
        .find(|package| package.name == dependency.name && package.source == dependency.source)
    else {
        return Ok(None);
    };

    let Some(dependency_path) = dependency.path.as_deref() else {
        if dependency.source.is_none() {
            return Err(format!(
                "invalid cargo metadata: path dependency `{}` has no `path`",
                dependency.name
            ));
        }
        return Ok(None);
    };
    let manifest_dir = package.manifest_path.parent().ok_or_else(|| {
        format!(
            "invalid cargo metadata: package `{}` has no manifest directory",
            package.name
        )
    })?;

    Ok((dependency_path == manifest_dir).then_some(package))
}
