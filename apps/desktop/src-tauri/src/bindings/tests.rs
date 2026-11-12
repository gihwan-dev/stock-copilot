use std::fs;
use std::path::{Path, PathBuf};

use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::Builder;

use super::{
    BINDINGS_FILE, COMMAND_LIST_FILE, COMMAND_SET_FILE, GenerateError, MOCK_BINDINGS_FILE,
    RESPONSES_FILE, SNAPSHOT_DIRECTORY, TAURI_DIRECTORY, generate,
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

#[derive(Type)]
struct WideI64 {
    value: i64,
}

#[derive(Type)]
struct WideU128 {
    value: u128,
}

#[derive(Type)]
struct WideI128 {
    value: i128,
}

#[derive(Type)]
struct WideUsize {
    value: usize,
}

#[derive(Type)]
struct WideIsize {
    value: isize,
}

fn refused(name: &str, builder: &Builder<tauri::Wry>) -> Option<String> {
    let root = fresh_root(&format!("bigint-{name}"));
    let exported = builder.export(Typescript::default(), root.join("bigint.ts"));
    let message = format!("{exported:?}");
    (exported.is_ok() || !message.contains("BigInt")).then(|| format!("{name}: {message}"))
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
fn every_bigint_width_is_refused_by_the_export() {
    assert_eq!(
        [
            WideI64 { value: 1 }.value.to_string(),
            WideU128 { value: 1 }.value.to_string(),
            WideI128 { value: 1 }.value.to_string(),
            WideUsize { value: 1 }.value.to_string(),
            WideIsize { value: 1 }.value.to_string(),
        ],
        ["1", "1", "1", "1", "1"],
        "sample values"
    );
    let exported: Vec<String> = [
        ("i64", Builder::<tauri::Wry>::new().typ::<WideI64>()),
        ("u128", Builder::<tauri::Wry>::new().typ::<WideU128>()),
        ("i128", Builder::<tauri::Wry>::new().typ::<WideI128>()),
        ("usize", Builder::<tauri::Wry>::new().typ::<WideUsize>()),
        ("isize", Builder::<tauri::Wry>::new().typ::<WideIsize>()),
    ]
    .into_iter()
    .filter_map(|(name, builder)| refused(name, &builder))
    .collect();
    assert!(exported.is_empty(), "exported: {exported:?}");
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

const INJECTION: &str =
    "0, injected: (() => { void fetch(\"https://example.invalid\"); return 1 as any; })()";

fn failure_kind(generated: &Result<(), GenerateError>) -> &'static str {
    match generated {
        Ok(()) => "ok",
        Err(GenerateError::Typescript(_)) => "typescript",
        Err(GenerateError::Io(..)) => "io",
        Err(GenerateError::Json(..)) => "json",
        Err(GenerateError::UnexpectedRecord(_)) => "unexpected-record",
        Err(GenerateError::MissingRecord(name)) if name == "ipc_errors" => "missing-ipc-errors",
        Err(GenerateError::MissingRecord(_)) => "missing-record",
    }
}

fn record(root: &Path, name: &str) -> PathBuf {
    root.join(SNAPSHOT_DIRECTORY).join(format!("{name}.json"))
}

#[test]
fn recorded_responses_are_reserialized_instead_of_pasted() {
    let root = fresh_root("reserialized");
    fs::write(
        record(&root, "app_version"),
        "{\n\n      \"version\"   :   \"0.1.0\"\n}\n",
    )
    .expect("write spaced record");
    generate(&root).expect("generate bindings");
    let responses = read(&root, Path::new(RESPONSES_FILE));
    assert!(
        responses.contains("appVersion: {\"version\":\"0.1.0\"},\n"),
        "{responses}"
    );
}

#[test]
fn a_record_that_is_not_json_fails_the_generation() {
    let root = fresh_root("not-json");
    fs::write(
        record(&root, "app_version"),
        format!("{{\"version\": \"0.1.0\"}}, {INJECTION}"),
    )
    .expect("write injected record");
    let generated = generate(&root);
    assert!(failure_kind(&generated) == "json", "{generated:?}");
    assert!(
        !Path::new(&root).join(RESPONSES_FILE).exists(),
        "no responses file is written"
    );
}

#[test]
fn a_record_outside_the_commands_and_fixed_cases_fails_the_generation() {
    let root = fresh_root("extra-record");
    fs::write(record(&root, "zz"), INJECTION).expect("write extra record");
    let generated = generate(&root);
    assert!(
        failure_kind(&generated) == "unexpected-record",
        "{generated:?}"
    );
}

#[test]
fn a_missing_record_fails_the_generation() {
    let root = fresh_root("missing-record");
    fs::remove_file(record(&root, "ipc_errors")).expect("remove record");
    let generated = generate(&root);
    assert!(
        failure_kind(&generated) == "missing-ipc-errors",
        "{generated:?}"
    );
}
