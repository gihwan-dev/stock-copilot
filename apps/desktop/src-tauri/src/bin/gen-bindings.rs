use std::path::Path;

use src_tauri::bindings::{GenerateError, generate};

fn main() -> Result<(), GenerateError> {
    generate(Path::new("."))
}
