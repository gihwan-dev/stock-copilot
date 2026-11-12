use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFailure(pub(crate) String);

impl fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn io_failure(path: &Path, error: &std::io::Error) -> SourceFailure {
    SourceFailure(format!("{}: {error}", path.display()))
}

#[derive(Debug, Clone)]
pub(crate) struct Member {
    pub(crate) name: String,
    pub(crate) directory: PathBuf,
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) dependencies: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Members {
    pub(crate) root: PathBuf,
    pub(crate) packages: Vec<Member>,
    pub(crate) target_directory: PathBuf,
}

pub(crate) fn load_members(manifest: Option<&Path>) -> Result<Members, SourceFailure> {
    let mut command = MetadataCommand::new();
    if let Some(path) = manifest {
        command.manifest_path(path);
    }
    let metadata = command
        .no_deps()
        .other_options(vec![String::from("--locked")])
        .exec()
        .map_err(|error| SourceFailure(format!("cargo metadata: {error}")))?;
    let packages = metadata
        .workspace_packages()
        .into_iter()
        .map(|package| Member {
            name: package.name.to_string(),
            directory: package
                .manifest_path
                .parent()
                .map_or_else(PathBuf::new, |parent| parent.as_std_path().to_path_buf()),
            roots: package
                .targets
                .iter()
                .filter(|target| !target.is_test() && !target.is_bench())
                .map(|target| target.src_path.as_std_path().to_path_buf())
                .collect(),
            dependencies: package
                .dependencies
                .iter()
                .filter(|dependency| {
                    dependency.path.is_some()
                        && dependency.kind == cargo_metadata::DependencyKind::Normal
                })
                .map(|dependency| dependency.name.clone())
                .collect(),
        })
        .collect();
    Ok(Members {
        root: metadata.workspace_root.into_std_path_buf(),
        packages,
        target_directory: metadata.target_directory.into_std_path_buf(),
    })
}

pub(crate) fn relative_text(root: &Path, path: &Path) -> String {
    let relative = match path.strip_prefix(root) {
        Ok(inside) => inside,
        Err(_) => path,
    };
    relative
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn has_rust_extension(path: &Path) -> Option<()> {
    path.extension()
        .is_some_and(|extension| extension == "rs")
        .then_some(())
}

fn list_directory(
    current: &Path,
    skipped: &BTreeSet<PathBuf>,
    pending: &mut Vec<PathBuf>,
    files: &mut Vec<PathBuf>,
) -> Result<(), SourceFailure> {
    let listing = fs::read_dir(current).map_err(|error| io_failure(current, &error))?;
    for listed in listing {
        let entry = listed.map_err(|error| io_failure(current, &error))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| io_failure(&path, &error))?;
        if kind.is_dir() {
            if !skipped.contains(&path) {
                pending.push(path);
            }
        } else if has_rust_extension(&path).is_some() {
            files.push(path);
        }
    }
    Ok(())
}

pub(crate) fn member_rust_files(members: &Members) -> Result<Vec<PathBuf>, SourceFailure> {
    let mut skipped: BTreeSet<PathBuf> = members
        .packages
        .iter()
        .map(|member| member.directory.clone())
        .collect();
    skipped.insert(members.target_directory.clone());
    let mut files = Vec::new();
    for member in &members.packages {
        let mut pending = vec![member.directory.clone()];
        while let Some(current) = pending.pop() {
            list_directory(&current, &skipped, &mut pending, &mut files)?;
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}
