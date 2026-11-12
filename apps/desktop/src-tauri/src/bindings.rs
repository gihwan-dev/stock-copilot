use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use specta::datatype::Function;
use specta_typescript::Typescript;
use tauri_specta::{BuilderConfiguration, LanguageExt};

use crate::ipc::registry::specta_builder;

const TAURI_DIRECTORY: &str = "apps/desktop/src-tauri";
const BINDINGS_FILE: &str = "apps/desktop/src/shared/ipc/bindings.ts";
const MOCK_BINDINGS_FILE: &str = "apps/desktop/src/shared/ipc/testing/mockBindings.ts";
const RESPONSES_FILE: &str = "apps/desktop/src/shared/ipc/testing/responses.ts";
const SNAPSHOT_DIRECTORY: &str = "apps/desktop/src-tauri/ipc/responses";
const COMMAND_LIST_FILE: &str = "ipc/commands.txt";
const COMMAND_SET_FILE: &str = "permissions/commands.toml";
const SNAPSHOT_SUFFIX: &str = ".json";
const ERROR_RECORD: &str = "ipc_errors";
const COMMAND_SET_HEADER: &str = "[[set]]\nidentifier = \"commands\"\ndescription = \"IPC 명령 허용 항목. gen-bindings가 ipc/commands.txt와 같은 목록에서 만든다\"\n";

#[derive(Debug)]
pub enum GenerateError {
    Typescript(specta_typescript::Error),
    Io(PathBuf, io::Error),
    Json(PathBuf, serde_json::Error),
    UnexpectedRecord(PathBuf),
    MissingRecord(String),
}

#[derive(Debug, Clone, Copy)]
struct CommandList;

impl LanguageExt for CommandList {
    type Error = io::Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        let mut names: Vec<&str> = cfg.commands.iter().map(Function::name).collect();
        names.sort_unstable();
        fs::write(path.join(COMMAND_LIST_FILE), command_list(&names))?;
        fs::write(path.join(COMMAND_SET_FILE), command_set(&names))
    }
}

fn command_list(names: &[&str]) -> String {
    names.iter().fold(String::new(), |mut text, name| {
        text.push_str(name);
        text.push('\n');
        text
    })
}

fn command_set(names: &[&str]) -> String {
    let permissions: Vec<String> = names
        .iter()
        .map(|name| format!("\"allow-{}\"", name.replace('_', "-")))
        .collect();
    format!(
        "{COMMAND_SET_HEADER}permissions = [{}]\n",
        permissions.join(", ")
    )
}

fn camel_case(name: &str) -> String {
    let mut text = String::new();
    let mut upper = false;
    for next in name.chars() {
        if next == '_' {
            upper = true;
        } else if upper {
            text.extend(next.to_uppercase());
            upper = false;
        } else {
            text.push(next);
        }
    }
    text
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> GenerateError + '_ {
    move |error| GenerateError::Io(path.to_path_buf(), error)
}

fn listed_records(directory: &Path) -> Result<BTreeSet<String>, GenerateError> {
    fs::read_dir(directory)
        .map_err(io_error(directory))?
        .map(|entry| {
            let path = entry.map_err(io_error(directory))?.path();
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(SNAPSHOT_SUFFIX))
                .filter(|_| path.is_file())
                .map(String::from)
                .ok_or(GenerateError::UnexpectedRecord(path.clone()))
        })
        .collect()
}

fn recorded(directory: &Path, name: &str) -> Result<String, GenerateError> {
    let file = directory.join(format!("{name}{SNAPSHOT_SUFFIX}"));
    let text = fs::read_to_string(&file).map_err(io_error(&file))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| GenerateError::Json(file.clone(), error))?;
    let canonical =
        serde_json::to_string(&value).map_err(|error| GenerateError::Json(file, error))?;
    Ok(format!("{}: {canonical},\n", camel_case(name)))
}

fn responses(root: &Path, commands: &[String]) -> Result<String, GenerateError> {
    let directory = root.join(SNAPSHOT_DIRECTORY);
    let expected: BTreeSet<String> = commands
        .iter()
        .cloned()
        .chain([String::from(ERROR_RECORD)])
        .collect();
    let listed = listed_records(&directory)?;
    if let Some(extra) = listed.difference(&expected).next() {
        return Err(GenerateError::UnexpectedRecord(
            directory.join(format!("{extra}{SNAPSHOT_SUFFIX}")),
        ));
    }
    if let Some(missing) = expected.difference(&listed).next() {
        return Err(GenerateError::MissingRecord(missing.clone()));
    }
    let entries = expected
        .iter()
        .map(|name| recorded(&directory, name))
        .collect::<Result<Vec<String>, GenerateError>>()?;
    Ok(format!(
        "export const responses = {{\n{}}} as const;\n",
        entries.concat()
    ))
}

pub fn generate(root: &Path) -> Result<(), GenerateError> {
    let builder = specta_builder::<tauri::Wry>();
    builder
        .export(Typescript::default(), root.join(BINDINGS_FILE))
        .map_err(GenerateError::Typescript)?;
    builder
        .clone()
        .mocks()
        .export(Typescript::default(), root.join(MOCK_BINDINGS_FILE))
        .map_err(GenerateError::Typescript)?;
    let tauri_directory = root.join(TAURI_DIRECTORY);
    builder
        .export(CommandList, &tauri_directory)
        .map_err(io_error(&tauri_directory))?;
    let list_file = tauri_directory.join(COMMAND_LIST_FILE);
    let commands: Vec<String> = fs::read_to_string(&list_file)
        .map_err(io_error(&list_file))?
        .lines()
        .map(String::from)
        .collect();
    let responses_file = root.join(RESPONSES_FILE);
    fs::write(&responses_file, responses(root, &commands)?).map_err(io_error(&responses_file))
}

#[cfg(test)]
mod tests;
