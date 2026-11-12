use std::error::Error;
use std::fs;
use std::io::{self, Write};

const COMMAND_LIST: &str = "ipc/commands.txt";

fn main() -> Result<(), Box<dyn Error>> {
    writeln!(io::stdout().lock(), "cargo:rerun-if-changed={COMMAND_LIST}")?;
    let listed = fs::read_to_string(COMMAND_LIST)?;
    let commands: Vec<&'static str> = listed
        .lines()
        .map(|line| &*String::from(line).leak())
        .collect();
    let manifest = tauri_build::AppManifest::new().commands(commands.leak());
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))?;
    Ok(())
}

