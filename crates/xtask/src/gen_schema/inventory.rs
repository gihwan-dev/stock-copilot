use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) const DIRECTORY: &str = "crates/adapters/broker-toss-schema";
pub(super) const FILES: [&str; 3] = ["Cargo.toml", "src/lib.rs", "src/types.rs"];

pub(super) fn check(root: &Path, mode: super::Mode) -> Result<(), String> {
    let directory = root.join(DIRECTORY);
    if fs::symlink_metadata(&directory)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err(String::from("생성 crate 심볼릭 링크 금지"));
    }
    let mut pending = vec![directory.clone()];
    while let Some(current) = pending.pop() {
        for listed in fs::read_dir(&current).map_err(|error| error.to_string())? {
            let entry = listed.map_err(|error| error.to_string())?;
            let path = entry.path();
            let relative = path
                .strip_prefix(&directory)
                .map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_dir() && relative == Path::new("src") {
                pending.push(path);
            } else if !fs::symlink_metadata(&path)
                .map_err(|error| error.to_string())?
                .is_file()
                || !FILES.iter().any(|name| relative == Path::new(name))
            {
                return Err(format!(
                    "{}: 생성 crate 추가 파일·폴더·링크 금지",
                    path.display()
                ));
            }
        }
    }
    tracked(root, mode)
}

fn tracked(root: &Path, mode: super::Mode) -> Result<(), String> {
    let output = crate::git_base::git(root)
        .args(["ls-files", "--cached", "-z", "--", DIRECTORY])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let allowed: BTreeSet<String> = FILES
        .iter()
        .map(|name| format!("{DIRECTORY}/{name}"))
        .collect();
    let names = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let mut found = BTreeSet::new();
    for name in names.split('\0').filter(|name| !name.is_empty()) {
        found.insert(String::from(name));
        if !allowed.contains(name) {
            return Err(format!("{name}: 생성 crate 추가 추적 파일 금지"));
        }
    }
    if mode == super::Mode::Check && found != allowed {
        return Err(String::from("생성 crate 추적 파일 집합이 세 파일과 다르다"));
    }
    Ok(())
}
