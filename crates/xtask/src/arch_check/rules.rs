use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use toml::{Table, Value};

use super::graph::{DefaultFeatures, DepKind};
use super::{CheckFailure, CrateName};

#[derive(Debug, Clone)]
pub(crate) struct Rules {
    pub(crate) graph: GraphRules,
    pub(crate) source: SourceRules,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphRules {
    pub(crate) edges: BTreeMap<CrateName, BTreeSet<CrateName>>,
    pub(crate) dev_edges: DevEdges,
    pub(crate) restricted: Vec<Restriction>,
    pub(crate) product_bin: ProductBin,
    pub(crate) external_locations: Vec<ExternalLocation>,
    pub(crate) external_allowlists: Vec<ExternalAllowlist>,
    pub(crate) feature_locations: Vec<FeatureLocation>,
    pub(crate) feature_allowlists: Vec<FeatureAllowlist>,
    pub(crate) location_exceptions: Vec<LocationException>,
}

#[derive(Debug, Clone)]
pub(crate) struct DevEdges {
    pub(crate) crates: BTreeMap<CrateName, BTreeSet<CrateName>>,
    pub(crate) other_crates: BTreeSet<CrateName>,
}

#[derive(Debug, Clone)]
pub(crate) struct Restriction {
    pub(crate) target: CrateName,
    pub(crate) dependents: Vec<AllowedDependent>,
    pub(crate) any_dependent_kinds: BTreeSet<DepKind>,
    pub(crate) transitive: TransitiveBan,
}

#[derive(Debug, Clone)]
pub(crate) struct AllowedDependent {
    pub(crate) dependent: CrateName,
    pub(crate) kinds: BTreeSet<DepKind>,
}

#[derive(Debug, Clone)]
pub(crate) enum TransitiveBan {
    Nothing,
    Listed(BTreeSet<CrateName>),
    AllExcept(BTreeSet<CrateName>),
}

#[derive(Debug, Clone)]
pub(crate) struct ProductBin {
    pub(crate) package: CrateName,
    pub(crate) forbidden: BTreeSet<CrateName>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalLocation {
    pub(crate) crates: BTreeSet<CrateName>,
    pub(crate) prefixes: Vec<String>,
    pub(crate) scope: LocationScope,
}

#[derive(Debug, Clone)]
pub(crate) enum LocationScope {
    AllowedIn(BTreeSet<CrateName>),
    ForbiddenIn(BTreeSet<CrateName>),
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalAllowlist {
    pub(crate) dependent: CrateName,
    pub(crate) allowed: BTreeSet<CrateName>,
}

#[derive(Debug, Clone)]
pub(crate) struct FeatureLocation {
    pub(crate) external: CrateName,
    pub(crate) features: BTreeSet<String>,
    pub(crate) allowed_in: BTreeSet<CrateName>,
}

#[derive(Debug, Clone)]
pub(crate) struct FeatureAllowlist {
    pub(crate) external: CrateName,
    pub(crate) dependent: CrateName,
    pub(crate) default_features: DefaultFeatures,
    pub(crate) features: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceRules {
    pub(crate) schema_surface_crates: BTreeSet<CrateName>,
    pub(crate) reexport_crates: BTreeSet<CrateName>,
    pub(crate) presence_trait: String,
    pub(crate) presence_impl_crates: BTreeSet<CrateName>,
    pub(crate) presence_crate: String,
    pub(crate) write_means: Vec<String>,
    pub(crate) order_words: Vec<String>,
    pub(crate) order_write_crates: BTreeSet<CrateName>,
    pub(crate) http_writes: Vec<HttpWrite>,
    pub(crate) process_crates: BTreeSet<CrateName>,
    pub(crate) socket_crates: BTreeSet<CrateName>,
    pub(crate) socket_identifiers: Vec<String>,
    pub(crate) socket_modules: Vec<String>,
    pub(crate) host_needle: String,
    pub(crate) host_crates: BTreeSet<CrateName>,
    pub(crate) host_files: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct HttpWrite {
    pub(crate) dependent: CrateName,
    pub(crate) allowed_files: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LocationException {
    pub(crate) dependents: BTreeSet<CrateName>,
    pub(crate) externals: BTreeSet<CrateName>,
    pub(crate) through: CrateName,
}

fn problem(message: String) -> CheckFailure {
    CheckFailure::Rules(message)
}

fn only_keys(table: &Table, context: &str, allowed: &[&str]) -> Result<(), CheckFailure> {
    match table.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(problem(format!("{context}: 모르는 키 {key}"))),
        None => Ok(()),
    }
}

fn value<'table>(table: &'table Table, key: &str) -> Result<&'table Value, CheckFailure> {
    table
        .get(key)
        .ok_or_else(|| problem(format!("{key} 키가 없다")))
}

fn sub_table<'table>(table: &'table Table, key: &str) -> Result<&'table Table, CheckFailure> {
    value(table, key)?
        .as_table()
        .ok_or_else(|| problem(format!("{key}는 표여야 한다")))
}

fn table_array<'table>(
    table: &'table Table,
    key: &str,
) -> Result<Vec<&'table Table>, CheckFailure> {
    value(table, key)?
        .as_array()
        .ok_or_else(|| problem(format!("{key}는 표 배열이어야 한다")))?
        .iter()
        .map(|item| {
            item.as_table()
                .ok_or_else(|| problem(format!("{key}의 항목은 표여야 한다")))
        })
        .collect()
}

fn text(table: &Table, key: &str) -> Result<String, CheckFailure> {
    value(table, key)?
        .as_str()
        .map(String::from)
        .ok_or_else(|| problem(format!("{key}는 문자열이어야 한다")))
}

fn texts_of(item: &Value, key: &str) -> Result<Vec<String>, CheckFailure> {
    item.as_array()
        .ok_or_else(|| problem(format!("{key}는 문자열 배열이어야 한다")))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(String::from)
                .ok_or_else(|| problem(format!("{key}의 항목은 문자열이어야 한다")))
        })
        .collect()
}

fn texts(table: &Table, key: &str) -> Result<Vec<String>, CheckFailure> {
    texts_of(value(table, key)?, key)
}

fn optional_texts(table: &Table, key: &str) -> Result<Vec<String>, CheckFailure> {
    match table.get(key) {
        Some(item) => texts_of(item, key),
        None => Ok(Vec::new()),
    }
}

fn names_of(entries: &[String]) -> BTreeSet<CrateName> {
    entries.iter().map(|entry| CrateName::new(entry)).collect()
}

fn names(table: &Table, key: &str) -> Result<BTreeSet<CrateName>, CheckFailure> {
    Ok(names_of(&texts(table, key)?))
}

fn name_map(table: &Table) -> Result<BTreeMap<CrateName, BTreeSet<CrateName>>, CheckFailure> {
    table
        .iter()
        .map(|(key, item)| Ok((CrateName::new(key), names_of(&texts_of(item, key)?))))
        .collect()
}

fn dep_kind(text: &str) -> Result<DepKind, CheckFailure> {
    match text {
        "normal" => Ok(DepKind::Normal),
        "build" => Ok(DepKind::Build),
        "dev" => Ok(DepKind::Dev),
        other => Err(problem(format!("모르는 의존 종류 {other}"))),
    }
}

fn dep_kinds(table: &Table, key: &str) -> Result<BTreeSet<DepKind>, CheckFailure> {
    optional_texts(table, key)?
        .iter()
        .map(|kind| dep_kind(kind))
        .collect()
}

fn dev_edges(table: &Table) -> Result<DevEdges, CheckFailure> {
    only_keys(table, "dev-edges", &["other-crates", "crates"])?;
    Ok(DevEdges {
        crates: name_map(sub_table(table, "crates")?)?,
        other_crates: names(table, "other-crates")?,
    })
}

fn allowed_dependent(table: &Table) -> Result<AllowedDependent, CheckFailure> {
    only_keys(table, "dependents", &["crate", "kinds"])?;
    Ok(AllowedDependent {
        dependent: CrateName::new(&text(table, "crate")?),
        kinds: dep_kinds(table, "kinds")?,
    })
}

fn transitive_ban(table: &Table) -> Result<TransitiveBan, CheckFailure> {
    match (
        table.get("transitive-forbidden"),
        table.get("transitive-forbidden-all-except"),
    ) {
        (None, None) => Ok(TransitiveBan::Nothing),
        (Some(listed), None) => Ok(TransitiveBan::Listed(names_of(&texts_of(
            listed,
            "transitive-forbidden",
        )?))),
        (None, Some(kept)) => Ok(TransitiveBan::AllExcept(names_of(&texts_of(
            kept,
            "transitive-forbidden-all-except",
        )?))),
        (Some(_), Some(_)) => Err(problem(String::from(
            "transitive-forbidden와 transitive-forbidden-all-except는 함께 쓸 수 없다",
        ))),
    }
}

fn restriction(table: &Table) -> Result<Restriction, CheckFailure> {
    only_keys(
        table,
        "restricted",
        &[
            "target",
            "dependents",
            "any-dependent-kinds",
            "transitive-forbidden",
            "transitive-forbidden-all-except",
        ],
    )?;
    Ok(Restriction {
        target: CrateName::new(&text(table, "target")?),
        dependents: table_array(table, "dependents")?
            .into_iter()
            .map(allowed_dependent)
            .collect::<Result<_, _>>()?,
        any_dependent_kinds: dep_kinds(table, "any-dependent-kinds")?,
        transitive: transitive_ban(table)?,
    })
}

fn product_bin(table: &Table) -> Result<ProductBin, CheckFailure> {
    only_keys(table, "product-bin", &["package", "forbidden"])?;
    Ok(ProductBin {
        package: CrateName::new(&text(table, "package")?),
        forbidden: names(table, "forbidden")?,
    })
}

fn external_location(table: &Table) -> Result<ExternalLocation, CheckFailure> {
    only_keys(
        table,
        "external-location",
        &["crates", "prefixes", "allowed-in", "forbidden-in"],
    )?;
    let scope = match (table.get("allowed-in"), table.get("forbidden-in")) {
        (Some(allowed), None) => {
            LocationScope::AllowedIn(names_of(&texts_of(allowed, "allowed-in")?))
        }
        (None, Some(forbidden)) => {
            LocationScope::ForbiddenIn(names_of(&texts_of(forbidden, "forbidden-in")?))
        }
        (None, None) | (Some(_), Some(_)) => {
            return Err(problem(String::from(
                "external-location에는 allowed-in과 forbidden-in 중 하나만 둔다",
            )));
        }
    };
    Ok(ExternalLocation {
        crates: names(table, "crates")?,
        prefixes: optional_texts(table, "prefixes")?,
        scope,
    })
}

fn external_allowlist(table: &Table) -> Result<ExternalAllowlist, CheckFailure> {
    only_keys(table, "external-allowlist", &["crate", "allowed"])?;
    Ok(ExternalAllowlist {
        dependent: CrateName::new(&text(table, "crate")?),
        allowed: names(table, "allowed")?,
    })
}

fn feature_location(table: &Table) -> Result<FeatureLocation, CheckFailure> {
    only_keys(
        table,
        "feature-location",
        &["external", "features", "allowed-in"],
    )?;
    Ok(FeatureLocation {
        external: CrateName::new(&text(table, "external")?),
        features: texts(table, "features")?.into_iter().collect(),
        allowed_in: names(table, "allowed-in")?,
    })
}

fn default_features(text: &str) -> Result<DefaultFeatures, CheckFailure> {
    match text {
        "on" => Ok(DefaultFeatures::On),
        "off" => Ok(DefaultFeatures::Off),
        other => Err(problem(format!(
            "default-features는 on 또는 off다: {other}"
        ))),
    }
}

fn feature_allowlist(table: &Table) -> Result<FeatureAllowlist, CheckFailure> {
    only_keys(
        table,
        "feature-allowlist",
        &["external", "dependent", "default-features", "features"],
    )?;
    Ok(FeatureAllowlist {
        external: CrateName::new(&text(table, "external")?),
        dependent: CrateName::new(&text(table, "dependent")?),
        default_features: default_features(&text(table, "default-features")?)?,
        features: texts(table, "features")?.into_iter().collect(),
    })
}

fn location_exception(table: &Table) -> Result<LocationException, CheckFailure> {
    only_keys(
        table,
        "external-location-exception",
        &["crates", "externals", "through", "reason"],
    )?;
    if text(table, "reason")?.trim().is_empty() {
        return Err(problem(String::from(
            "external-location-exception에는 이유가 있어야 한다",
        )));
    }
    Ok(LocationException {
        dependents: names(table, "crates")?,
        externals: names(table, "externals")?,
        through: CrateName::new(&text(table, "through")?),
    })
}

fn http_write(table: &Table) -> Result<HttpWrite, CheckFailure> {
    only_keys(table, "source.http-write", &["crate", "allowed-files"])?;
    Ok(HttpWrite {
        dependent: CrateName::new(&text(table, "crate")?),
        allowed_files: texts(table, "allowed-files")?.into_iter().collect(),
    })
}

fn each<T>(
    table: &Table,
    key: &str,
    parse: fn(&Table) -> Result<T, CheckFailure>,
) -> Result<Vec<T>, CheckFailure> {
    table_array(table, key)?.into_iter().map(parse).collect()
}

fn graph_rules(table: &Table) -> Result<GraphRules, CheckFailure> {
    Ok(GraphRules {
        edges: name_map(sub_table(table, "edges")?)?,
        dev_edges: dev_edges(sub_table(table, "dev-edges")?)?,
        restricted: each(table, "restricted", restriction)?,
        product_bin: product_bin(sub_table(table, "product-bin")?)?,
        external_locations: each(table, "external-location", external_location)?,
        external_allowlists: each(table, "external-allowlist", external_allowlist)?,
        feature_locations: each(table, "feature-location", feature_location)?,
        feature_allowlists: each(table, "feature-allowlist", feature_allowlist)?,
        location_exceptions: match table.get("external-location-exception") {
            Some(_) => each(table, "external-location-exception", location_exception)?,
            None => Vec::new(),
        },
    })
}

fn schema_surface_names(table: &Table) -> Result<BTreeSet<CrateName>, CheckFailure> {
    let schema = sub_table(table, "schema-surface")?;
    only_keys(schema, "source.schema-surface", &["crates"])?;
    names(schema, "crates")
}

fn source_rules(table: &Table) -> Result<SourceRules, CheckFailure> {
    only_keys(
        table,
        "source",
        &[
            "reexport",
            "schema-surface",
            "user-presence",
            "toss-order",
            "http-write",
            "process",
            "http-socket",
            "toss-host",
        ],
    )?;
    let reexport = sub_table(table, "reexport")?;
    only_keys(reexport, "source.reexport", &["crates"])?;
    let presence = sub_table(table, "user-presence")?;
    only_keys(
        presence,
        "source.user-presence",
        &["trait-name", "impl-crates", "defining-crate"],
    )?;
    let order = sub_table(table, "toss-order")?;
    only_keys(
        order,
        "source.toss-order",
        &["write-means", "order-words", "allowed-crates"],
    )?;
    let process = sub_table(table, "process")?;
    only_keys(process, "source.process", &["allowed-crates"])?;
    let socket = sub_table(table, "http-socket")?;
    only_keys(
        socket,
        "source.http-socket",
        &["crates", "identifiers", "module-paths"],
    )?;
    let host = sub_table(table, "toss-host")?;
    only_keys(
        host,
        "source.toss-host",
        &["needle", "allowed-crates", "allowed-files"],
    )?;
    Ok(SourceRules {
        schema_surface_crates: schema_surface_names(table)?,
        reexport_crates: names(reexport, "crates")?,
        presence_trait: text(presence, "trait-name")?,
        presence_impl_crates: names(presence, "impl-crates")?,
        presence_crate: text(presence, "defining-crate")?,
        write_means: texts(order, "write-means")?,
        order_words: texts(order, "order-words")?,
        order_write_crates: names(order, "allowed-crates")?,
        http_writes: each(table, "http-write", http_write)?,
        process_crates: names(process, "allowed-crates")?,
        socket_crates: names(socket, "crates")?,
        socket_identifiers: texts(socket, "identifiers")?,
        socket_modules: texts(socket, "module-paths")?,
        host_needle: text(host, "needle")?,
        host_crates: names(host, "allowed-crates")?,
        host_files: texts(host, "allowed-files")?.into_iter().collect(),
    })
}

pub(crate) fn parse_rules(content: &str) -> Result<Rules, CheckFailure> {
    let table: Table = content
        .parse()
        .map_err(|error| problem(format!("TOML 해석 실패: {error}")))?;
    only_keys(
        &table,
        "최상위",
        &[
            "edges",
            "dev-edges",
            "restricted",
            "product-bin",
            "external-location",
            "external-allowlist",
            "feature-location",
            "feature-allowlist",
            "external-location-exception",
            "source",
        ],
    )?;
    Ok(Rules {
        graph: graph_rules(&table)?,
        source: source_rules(sub_table(&table, "source")?)?,
    })
}

pub(crate) fn load_rules(path: &Path) -> Result<Rules, CheckFailure> {
    let content = fs::read_to_string(path).map_err(|error| CheckFailure::Io {
        path: path.to_path_buf(),
        error,
    })?;
    parse_rules(&content)
}
