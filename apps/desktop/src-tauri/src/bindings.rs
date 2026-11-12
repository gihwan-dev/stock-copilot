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
const COMMAND_SET_HEADER: &str = "[[set]]\nidentifier = \"commands\"\ndescription = \"IPC 명령 허용 항목. gen-bindings가 ipc/commands.txt와 같은 목록에서 만든다\"\n";

#[derive(Debug)]
pub enum GenerateError {
    Typescript(specta_typescript::Error),
    Io(PathBuf, io::Error),
    SnapshotName(PathBuf),
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

fn snapshot_key(path: &Path) -> Result<String, GenerateError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(SNAPSHOT_SUFFIX))
        .map(camel_case)
        .ok_or_else(|| GenerateError::SnapshotName(path.to_path_buf()))
}

fn responses(root: &Path) -> Result<String, GenerateError> {
    let directory = root.join(SNAPSHOT_DIRECTORY);
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .map_err(io_error(&directory))?
        .map(|entry| {
            entry
                .map(|found| found.path())
                .map_err(io_error(&directory))
        })
        .collect::<Result<_, _>>()?;
    files.retain(|file| {
        file.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(SNAPSHOT_SUFFIX))
    });
    files.sort();
    let entries = files
        .iter()
        .map(|file| {
            let content = fs::read_to_string(file).map_err(io_error(file))?;
            Ok(format!("{}: {},\n", snapshot_key(file)?, content.trim()))
        })
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
    let responses_file = root.join(RESPONSES_FILE);
    fs::write(&responses_file, responses(root)?).map_err(io_error(&responses_file))
}

#[cfg(test)]
mod tests;
