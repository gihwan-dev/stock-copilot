use std::path::Path;
use std::process::Command;

use super::IgnoreFiles;

pub(super) const FILE: &str = "catalog.json";

pub(super) fn prepare(
    root: &Path,
    globs: &[String],
    ignore: IgnoreFiles,
    output: &Path,
) -> Result<(), String> {
    let receipt = output.join(FILE);
    if receipt.exists() {
        std::fs::remove_file(&receipt).map_err(|error| error.to_string())?;
    }
    let mut command = Command::new("cargo");
    command.current_dir(root).args([
        "mutants",
        "--no-config",
        "--cap-lints=true",
        "--test-tool=nextest",
        "--no-shuffle",
        "--list",
        "--json",
    ]);
    for glob in globs {
        command.arg("--file").arg(glob);
    }
    if ignore == IgnoreFiles::Disregarded {
        command.arg("--gitignore=false");
    }
    let listed = command.output().map_err(|error| error.to_string())?;
    if !listed.status.success() {
        return Err(format!(
            "밤 전체 변이 열거 실패: {}",
            String::from_utf8_lossy(&listed.stderr)
        ));
    }
    validate(&listed.stdout)?;
    std::fs::write(receipt, listed.stdout).map_err(|error| error.to_string())
}

fn validate(bytes: &[u8]) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let entries = value.as_array().ok_or("밤 전체 변이 목록이 아니다")?;
    if entries.is_empty() {
        return Err(String::from("밤 전체 변이 목록이 비어 있다"));
    }
    if entries.iter().all(|entry| {
        ["name", "file"].iter().all(|field| {
            entry
                .get(field)
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| !text.is_empty())
        })
    }) {
        Ok(())
    } else {
        Err(String::from("밤 전체 변이 목록에 name 또는 file이 없다"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_requires_a_nonempty_array_with_each_identity_field() {
        for text in [
            "[]",
            "{}",
            "{",
            "[null]",
            "[{\"name\":\"good\",\"file\":\"src/good.rs\"},null]",
            "[{}]",
            "[{\"name\":\"n\"}]",
            "[{\"file\":\"f.rs\"}]",
            "[{\"name\":\"\",\"file\":\"f.rs\"}]",
            "[{\"name\":\"n\",\"file\":\"\"}]",
        ] {
            super::validate(text.as_bytes()).expect_err("invalid catalog");
        }
        super::validate(br#"[{"name":"src/a.rs:12:9: replace >","file":"src/a.rs"},{"name":"src/b.rs:21:3: replace +","file":"src/b.rs"}]"#).expect("distinct catalog entries");
    }
}
