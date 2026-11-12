fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&["get_holdings"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("tauri build");
}
