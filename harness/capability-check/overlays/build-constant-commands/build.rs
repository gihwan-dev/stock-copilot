const GET_HOLDINGS: &str = "get_holdings";

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[GET_HOLDINGS]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("tauri build");
}
