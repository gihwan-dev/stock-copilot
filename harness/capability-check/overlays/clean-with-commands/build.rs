const COMMAND_LIST: &str = "ipc/commands.txt";

fn main() {
    println!("cargo:rerun-if-changed={COMMAND_LIST}");
    let listed = std::fs::read_to_string(COMMAND_LIST).expect("read command list");
    let commands: Vec<&'static str> = listed
        .lines()
        .map(|line| &*Box::leak(line.to_owned().into_boxed_str()))
        .collect();
    let manifest = tauri_build::AppManifest::new().commands(Box::leak(commands.into_boxed_slice()));
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("tauri build");
}
