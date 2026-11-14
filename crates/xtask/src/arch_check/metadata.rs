use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cargo_metadata::{CargoOpt, DependencyKind, Metadata, MetadataCommand, Package, PackageId};

use super::graph::{
    CrateGraph, Declaration, DefaultFeatures, DepKind, Edge, Node, NodeKey, Origin, Provenance,
};
use super::{CheckFailure, CrateName};

#[derive(Debug, Clone)]
pub(crate) struct MemberPackage {
    pub(crate) name: CrateName,
    pub(crate) directory: PathBuf,
    pub(crate) schema_names: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) graph: CrateGraph,
    pub(crate) packages: Vec<MemberPackage>,
    pub(crate) target_directory: PathBuf,
}

const fn dep_kind(kind: DependencyKind) -> DepKind {
    match kind {
        DependencyKind::Development => DepKind::Dev,
        DependencyKind::Build => DepKind::Build,
        DependencyKind::Normal | DependencyKind::Unknown => DepKind::Normal,
    }
}

fn declarations(package: &Package) -> Vec<Declaration> {
    package
        .dependencies
        .iter()
        .map(|dependency| Declaration {
            dependency: CrateName::new(&dependency.name),
            default_features: if dependency.uses_default_features {
                DefaultFeatures::On
            } else {
                DefaultFeatures::Off
            },
            features: dependency.features.iter().cloned().collect(),
        })
        .collect()
}

fn node_key(id: &PackageId) -> NodeKey {
    NodeKey::new(&id.repr)
}

fn edges(dependencies: &[cargo_metadata::NodeDep]) -> Vec<Edge> {
    dependencies
        .iter()
        .flat_map(|dependency| {
            let kinds: BTreeSet<DepKind> = if dependency.dep_kinds.is_empty() {
                BTreeSet::from([DepKind::Normal])
            } else {
                dependency
                    .dep_kinds
                    .iter()
                    .map(|info| dep_kind(info.kind))
                    .collect()
            };
            kinds.into_iter().map(|kind| Edge {
                to: node_key(&dependency.pkg),
                kind,
            })
        })
        .collect()
}

fn graph(metadata: &Metadata) -> Result<CrateGraph, CheckFailure> {
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| CheckFailure::Graph(String::from("resolve 그래프가 없다")))?;
    let packages: BTreeMap<&PackageId, &Package> = metadata
        .packages
        .iter()
        .map(|package| (&package.id, package))
        .collect();
    let members: BTreeSet<&PackageId> = metadata.workspace_members.iter().collect();
    let nodes = resolve
        .nodes
        .iter()
        .map(|node| {
            let package = packages.get(&node.id).ok_or_else(|| {
                CheckFailure::Graph(format!("패키지 정보가 없다: {}", node.id.repr))
            })?;
            Ok((
                node_key(&node.id),
                Node {
                    name: CrateName::new(&package.name),
                    origin: if members.contains(&node.id) {
                        Origin::Member
                    } else {
                        Origin::External
                    },
                    provenance: match package.source {
                        Some(_) => Provenance::Sourced,
                        None => Provenance::Path,
                    },
                    edges: edges(&node.deps),
                    declarations: declarations(package),
                },
            ))
        })
        .collect::<Result<_, CheckFailure>>()?;
    Ok(CrateGraph { nodes })
}

fn member_packages(metadata: &Metadata) -> Vec<MemberPackage> {
    metadata
        .workspace_packages()
        .into_iter()
        .filter_map(|package| {
            package
                .manifest_path
                .parent()
                .map(|directory| MemberPackage {
                    name: CrateName::new(&package.name),
                    directory: directory.as_std_path().to_path_buf(),
                    schema_names: package
                        .dependencies
                        .iter()
                        .filter(|dependency| dependency.name == "broker-toss-schema")
                        .map(|dependency| {
                            dependency
                                .rename
                                .as_deref()
                                .map_or(dependency.name.as_str(), |name| name)
                                .replace('-', "_")
                        })
                        .collect(),
                })
        })
        .collect()
}

pub(crate) fn load_workspace(
    manifest: &Path,
    cargo_options: &[String],
) -> Result<Workspace, CheckFailure> {
    let mut options = vec![String::from("--locked"), String::from("--offline")];
    options.extend_from_slice(cargo_options);
    let metadata = MetadataCommand::new()
        .manifest_path(manifest)
        .features(CargoOpt::AllFeatures)
        .other_options(options)
        .exec()
        .map_err(CheckFailure::Metadata)?;
    Ok(Workspace {
        root: metadata.workspace_root.as_std_path().to_path_buf(),
        graph: graph(&metadata)?,
        packages: member_packages(&metadata),
        target_directory: metadata.target_directory.as_std_path().to_path_buf(),
    })
}
