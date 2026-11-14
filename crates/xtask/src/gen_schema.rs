mod normalize;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use serde_json::Value;
use sha2::{Digest, Sha256};

const INPUT: &str = "docs/toss-openapi-1.2.28.json";
const OUTPUT: &str = "crates/adapters/broker-toss-schema/src";

fn conversions(value: &Value, settings: &mut typify::TypeSpaceSettings) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            if map
                .get("format")
                .and_then(Value::as_str)
                .is_some_and(|format| ["date", "date-time", "uri"].contains(&format))
            {
                let object =
                    serde_json::from_value::<schemars::schema::SchemaObject>(value.clone())
                        .map_err(|error| error.to_string())?;
                let _settings =
                    settings.with_conversion(object, "::std::string::String", std::iter::empty());
            }
            for child in map.values() {
                conversions(child, settings)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                conversions(child, settings)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn references(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                *reference = reference.replace("#/components/schemas/", "#/definitions/");
            }
            for child in map.values_mut() {
                references(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                references(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn formatted(source: &str) -> Result<String, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    child
        .stdin
        .take()
        .ok_or_else(|| String::from("rustfmt stdin"))?
        .write_all(source.as_bytes())
        .map_err(|error| error.to_string())?;
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

fn render(input: &str) -> Result<[String; 2], String> {
    let spec: Value = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let mut schemas = spec
        .pointer("/components/schemas")
        .ok_or_else(|| String::from("components.schemas 없음"))?
        .clone();
    references(&mut schemas);
    let mut settings = typify::TypeSpaceSettings::default();
    conversions(&schemas, &mut settings)?;
    let definitions: BTreeMap<String, schemars::schema::Schema> =
        serde_json::from_value(schemas).map_err(|error| error.to_string())?;
    let mut space = typify::TypeSpace::new(&settings);
    space
        .add_ref_types(definitions)
        .map_err(|error| error.to_string())?;
    let types = formatted(&normalize::normalize(space.to_stream())?)?;
    let hash = Sha256::digest(input.as_bytes());
    let library = formatted(&format!(
        "pub mod types;\npub const SPEC_SHA256: &str = \"{hash:x}\";\n"
    ))?;
    Ok([library, types])
}

#[derive(Clone, Copy)]
enum Mode {
    Write,
    Check,
}

fn generate(root: &Path, mode: Mode) -> Result<(), String> {
    let input = fs::read_to_string(root.join(INPUT)).map_err(|error| error.to_string())?;
    let outputs = render(&input)?;
    for (name, content) in ["lib.rs", "types.rs"].into_iter().zip(outputs) {
        let path = root.join(OUTPUT).join(name);
        match mode {
            Mode::Write => fs::write(&path, content).map_err(|error| error.to_string())?,
            Mode::Check => {
                if fs::read_to_string(&path).map_err(|error| error.to_string())? != content {
                    return Err(format!(
                        "{}: 재생성 차이, xtask gen-schema를 실행하세요",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    let mode = match arguments {
        [] => Mode::Write,
        [flag] if flag == "--check" => Mode::Check,
        _ => return ExitCode::FAILURE,
    };
    let result = generate(Path::new("."), mode);
    match result {
        Ok(()) => {
            let _written = writeln!(io::stdout().lock(), "gen-schema: 위반 없음");
            ExitCode::SUCCESS
        }
        Err(error) => {
            let _written = writeln!(io::stderr().lock(), "gen-schema: {error}");
            ExitCode::FAILURE
        }
    }
}
