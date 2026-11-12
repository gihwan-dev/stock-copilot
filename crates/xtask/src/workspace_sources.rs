use std::fmt;
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
    pub(crate) roots: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct Members {
    pub(crate) root: PathBuf,
    pub(crate) packages: Vec<Member>,
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
            roots: package
                .targets
                .iter()
                .filter(|target| !target.is_test() && !target.is_bench())
                .map(|target| target.src_path.as_std_path().to_path_buf())
                .collect(),
        })
        .collect();
    Ok(Members {
        root: metadata.workspace_root.into_std_path_buf(),
        packages,
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
