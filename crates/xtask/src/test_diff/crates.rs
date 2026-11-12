use std::collections::BTreeSet;

use super::Files;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Root {
    pub(crate) binary: String,
    pub(crate) file: String,
}

fn manifest(files: &Files, path: &str) -> Option<toml::Table> {
    String::from_utf8_lossy(files.get(path)?).parse().ok()
}

fn members(files: &Files) -> Vec<String> {
    let Some(root) = manifest(files, "Cargo.toml") else {
        return Vec::new();
    };
    let listed: Vec<String> = root
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .map_or_else(Vec::new, |items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(String::from)
                .collect()
        });
    let mut found: BTreeSet<String> = BTreeSet::new();
    for member in listed {
        if let Some(parent) = member.strip_suffix("/*") {
            let prefix = format!("{parent}/");
            found.extend(files.keys().filter_map(|path| {
                let rest = path.strip_prefix(&prefix)?;
                let (directory, file) = rest.split_once('/')?;
                (file == "Cargo.toml").then(|| format!("{prefix}{directory}"))
            }));
        } else {
            found.insert(member);
        }
    }
    found.into_iter().collect()
}

fn listed_targets(table: &toml::Table, key: &str) -> Vec<(Option<String>, Option<String>)> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .map_or_else(Vec::new, |items| {
            items
                .iter()
                .map(|item| {
                    (
                        item.get("name")
                            .and_then(toml::Value::as_str)
                            .map(String::from),
                        item.get("path")
                            .and_then(toml::Value::as_str)
                            .map(String::from),
                    )
                })
                .collect()
        })
}

fn stem(path: &str) -> Option<&str> {
    path.rsplit('/').next()?.strip_suffix(".rs")
}

fn auto_targets(files: &Files, directory: &str) -> Vec<(String, String)> {
    let prefix = format!("{directory}/");
    files
        .keys()
        .filter_map(|path| {
            let rest = path.strip_prefix(&prefix)?;
            match rest.split('/').collect::<Vec<_>>().as_slice() {
                [file] => stem(file).map(|name| (String::from(name), path.clone())),
                [folder, "main.rs"] => Some((String::from(*folder), path.clone())),
                _ => None,
            }
        })
        .collect()
}

fn member_roots(files: &Files, member: &str) -> Vec<Root> {
    let Some(table) = manifest(files, &format!("{member}/Cargo.toml")) else {
        return Vec::new();
    };
    let Some(package) = table
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(String::from)
    else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    let library = table
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
        .map_or_else(|| String::from("src/lib.rs"), String::from);
    roots.push(Root {
        binary: package.clone(),
        file: format!("{member}/{library}"),
    });
    let mut binaries: Vec<(String, String)> =
        vec![(package.clone(), format!("{member}/src/main.rs"))];
    binaries.extend(auto_targets(files, &format!("{member}/src/bin")));
    for (name, path) in listed_targets(&table, "bin") {
        let named = name.unwrap_or_else(|| package.clone());
        let file = path.map_or_else(
            || format!("{member}/src/bin/{named}.rs"),
            |listed| format!("{member}/{listed}"),
        );
        binaries.retain(|(existing, _)| *existing != named);
        binaries.push((named, file));
    }
    roots.extend(binaries.into_iter().map(|(name, file)| Root {
        binary: format!("{package}::bin/{name}"),
        file,
    }));
    let mut tests = auto_targets(files, &format!("{member}/tests"));
    for (name, path) in listed_targets(&table, "test") {
        if let (Some(named), Some(listed)) = (name, path) {
            tests.retain(|(existing, _)| *existing != named);
            tests.push((named, format!("{member}/{listed}")));
        }
    }
    roots.extend(tests.into_iter().map(|(name, file)| Root {
        binary: format!("{package}::{name}"),
        file,
    }));
    roots
        .into_iter()
        .filter(|root| files.contains_key(&root.file))
        .collect()
}

pub(crate) fn crate_roots(files: &Files) -> Vec<Root> {
    let mut roots: Vec<Root> = members(files)
        .iter()
        .flat_map(|member| member_roots(files, member))
        .collect();
    roots.sort();
    roots.dedup();
    roots
}
