use std::fs;
use std::path::{Path, PathBuf};

use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::Builder;

use super::{
    BINDINGS_FILE, COMMAND_LIST_FILE, COMMAND_SET_FILE, MOCK_BINDINGS_FILE, RESPONSES_FILE,
    SNAPSHOT_DIRECTORY, TAURI_DIRECTORY, generate,
};

const REPOSITORY_ROOT: &str = "../../..";

#[derive(Type)]
struct LargeCount {
    count: u64,
}

impl LargeCount {
    const fn count(&self) -> u64 {
        self.count
    }
}

fn fresh_root(name: &str) -> PathBuf {
    let directory = Path::new(REPOSITORY_ROOT)
        .join("target")
        .join("src-tauri-tests")
        .join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("remove previous directory");
    }
    for folder in [
        Path::new(TAURI_DIRECTORY).join("ipc"),
        Path::new(TAURI_DIRECTORY).join("permissions"),
        Path::new(MOCK_BINDINGS_FILE)
            .parent()
            .expect("testing folder")
            .to_path_buf(),
        PathBuf::from(SNAPSHOT_DIRECTORY),
    ] {
        fs::create_dir_all(directory.join(folder)).expect("create folder");
    }
    for listed in fs::read_dir(Path::new(REPOSITORY_ROOT).join(SNAPSHOT_DIRECTORY))
        .expect("read recorded responses")
    {
        let entry = listed.expect("response entry");
        fs::copy(
            entry.path(),
            directory.join(SNAPSHOT_DIRECTORY).join(entry.file_name()),
        )
        .expect("copy recorded response");
    }
    directory
}

fn read(root: &Path, file: &Path) -> String {
    fs::read_to_string(root.join(file)).expect("read generated file")
}

#[test]
fn bigint_types_are_refused_by_the_export() {
    let root = fresh_root("bigint");
    assert_eq!(LargeCount { count: 1 }.count(), 1, "sample value");
    let exported = Builder::<tauri::Wry>::new()
        .typ::<LargeCount>()
        .export(Typescript::default(), root.join("bigint.ts"));
    let message = format!("{exported:?}");
    assert!(
        exported.is_err() && message.contains("BigInt"),
        "u64 field was exported: {message}"
    );
}

#[test]
fn regenerated_command_files_match_the_committed_files() {
    let root = fresh_root("generate");
    generate(&root).expect("generate bindings");
    for file in [COMMAND_LIST_FILE, COMMAND_SET_FILE] {
        let relative = Path::new(TAURI_DIRECTORY).join(file);
        assert_eq!(
            read(&root, &relative),
            read(Path::new(REPOSITORY_ROOT), &relative),
            "{} is stale; run just gen-bindings",
            relative.display()
        );
    }
}

#[test]
fn generated_typescript_names_the_command_mocks_and_recorded_responses() {
    let root = fresh_root("typescript");
    generate(&root).expect("generate bindings");
    let bindings = read(&root, Path::new(BINDINGS_FILE));
    let mocks = read(&root, Path::new(MOCK_BINDINGS_FILE));
    let responses = read(&root, Path::new(RESPONSES_FILE));
    assert!(
        bindings.contains("appVersion: () => typedError<AppVersion, IpcError>"),
        "{bindings}"
    );
    assert!(!bindings.contains("mockCommands"), "{bindings}");
    assert!(
        mocks.contains("export type MockCommandHandlers")
            && mocks.contains("export function mockCommands"),
        "{mocks}"
    );
    assert!(
        responses.starts_with("export const responses = {\n")
            && responses.contains("\nappVersion: {")
            && responses.contains("ipcErrors: [")
            && responses.ends_with("} as const;\n"),
        "{responses}"
    );
}
