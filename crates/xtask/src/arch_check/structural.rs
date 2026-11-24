#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprCall, Item};

use super::metadata::{MemberPackage, Workspace};
use super::rules::{IdentityConfirmationRules, Rules};
use super::source::identity_confirmation::{is_cfg_test_mod, is_direct_canonical_issuer_path};
use super::{CheckFailure, CrateName, Finding, Rule};

const STRUCTURAL_CACHE_DIR: &str = "arch-check-clippy";
const CONF_SUBDIR: &str = "conf";
const EMPTY_DIST_SUBDIR: &str = "empty-dist";
const LINUX_EXCLUSIONS_PATH: &str = "harness/ci/linux/exclusions.json";
const ISSUER_DISALLOWED_REASON: &str = "stock-copilot-h66-issuer";
const SEAL_DISALLOWED_REASON: &str = "stock-copilot-h66-seal";

const CLEARED_ENV_VARS: [&str; 37] = [
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS",
    "CARGO_TARGET_X86_64_APPLE_DARWIN_RUSTFLAGS",
    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
    "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
    "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER",
    "CARGO_TARGET_X86_64_APPLE_DARWIN_RUNNER",
    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER",
    "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER",
    "RUSTC",
    "CARGO_BUILD_RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_BOOTSTRAP",
    "CARGO_BUILD_TARGET",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "CLIPPY_CONF_DIR",
    "CLIPPY_ARGS",
    "CARGO_ALIAS_CLIPPY",
    "CARGO_ENV_CLIPPY_ARGS",
    "CARGO_ENV_CLIPPY_ARGS_VALUE",
    "CARGO_ENV_CLIPPY_ARGS_FORCE",
    "CARGO_ENV_CLIPPY_CONF_DIR",
    "CARGO_ENV_CLIPPY_CONF_DIR_VALUE",
    "CARGO_ENV_CLIPPY_CONF_DIR_FORCE",
    "CARGO_ENV_RUSTFLAGS",
    "CARGO_ENV_RUSTFLAGS_VALUE",
    "CARGO_ENV_RUSTFLAGS_FORCE",
    "CARGO_ENV_RUSTC_WRAPPER",
    "CARGO_ENV_RUSTC_WRAPPER_VALUE",
    "CARGO_ENV_RUSTC_WRAPPER_FORCE",
    "TAURI_CONFIG",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineRange {
    start: usize,
    end: usize,
}

impl LineRange {
    const fn contains(self, line_start: usize, line_end: usize) -> bool {
        line_start >= self.start && line_end <= self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpanRange {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

impl SpanRange {
    fn from_syn(span: proc_macro2::Span) -> Self {
        let start = span.start();
        let end = span.end();
        Self {
            start_line: start.line,
            start_column: start.column.saturating_add(1),
            end_line: end.line,
            end_column: end.column.saturating_add(1),
        }
    }

    const fn contains(self, other: Self) -> bool {
        let starts_after = other.start_line > self.start_line
            || (other.start_line == self.start_line && other.start_column >= self.start_column);
        let ends_before = other.end_line < self.end_line
            || (other.end_line == self.end_line && other.end_column <= self.end_column);
        starts_after && ends_before
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedCanary {
    Expected,
    NotExpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagnosticKind {
    IssuerMethod,
    SealType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandStatus {
    Succeeded,
    Failed,
}

enum SpanLocation {
    Workspace(String),
    External(String),
}

struct AllowedStructuralSites {
    defining_file: String,
    defining_test_file: String,
    defining_test_spans: Vec<SpanRange>,
    seal_struct_ranges: Vec<LineRange>,
    confirmation_struct_ranges: Vec<LineRange>,
    defining_test_file_ranges: Vec<LineRange>,
    allowed_direct_call_spans: BTreeMap<String, Vec<SpanRange>>,
    expected_issuer_packages: BTreeSet<CrateName>,
}

struct DirectIssuerCallCollector<'a> {
    rules: &'a IdentityConfirmationRules,
    spans: Vec<SpanRange>,
}

impl<'ast> Visit<'ast> for DirectIssuerCallCollector<'_> {
    fn visit_expr_call(&mut self, i: &'ast ExprCall) {
        if i.args.is_empty()
            && let Expr::Path(callee) = i.func.as_ref()
            && is_direct_canonical_issuer_path(self.rules, callee)
        {
            self.spans.push(SpanRange::from_syn(callee.span()));
        }
        visit::visit_expr_call(self, i);
    }
}

struct DefiningSites {
    test_spans: Vec<SpanRange>,
    seal_ranges: Vec<LineRange>,
    confirmation_ranges: Vec<LineRange>,
    canary: ExpectedCanary,
}

struct ParsedOutput {
    observed_packages: BTreeSet<CrateName>,
    compiler_errors: Vec<String>,
    seen_issuer_packages: BTreeSet<CrateName>,
    seen_seal: usize,
    findings: BTreeSet<Finding>,
}

fn io_failure(path: &Path, error: std::io::Error) -> CheckFailure {
    CheckFailure::Io {
        path: path.to_path_buf(),
        error,
    }
}

fn structural_error(message: impl Into<String>) -> CheckFailure {
    CheckFailure::Structural(message.into())
}

fn canonical_or_original(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) => path.to_path_buf(),
    }
}

fn item_line_range(attrs: &[Attribute], item_span: proc_macro2::Span) -> LineRange {
    let mut start = item_span.start().line;
    let mut end = item_span.end().line;
    for attr in attrs {
        let attr_span = attr.span();
        start = start.min(attr_span.start().line);
        end = end.max(attr_span.end().line);
    }
    LineRange { start, end }
}

fn item_span_range(attrs: &[Attribute], item_span: proc_macro2::Span) -> SpanRange {
    let mut result = SpanRange::from_syn(item_span);
    for attr in attrs {
        let attr_range = SpanRange::from_syn(attr.span());
        if attr_range.start_line < result.start_line
            || (attr_range.start_line == result.start_line
                && attr_range.start_column < result.start_column)
        {
            result.start_line = attr_range.start_line;
            result.start_column = attr_range.start_column;
        }
        if attr_range.end_line > result.end_line
            || (attr_range.end_line == result.end_line && attr_range.end_column > result.end_column)
        {
            result.end_line = attr_range.end_line;
            result.end_column = attr_range.end_column;
        }
    }
    result
}

fn defining_package_for_file<'ws>(
    workspace: &'ws Workspace,
    defining_file: &str,
) -> Option<&'ws MemberPackage> {
    let target = workspace.root.join(defining_file);
    workspace
        .packages
        .iter()
        .find(|package| target.starts_with(&package.directory))
}

fn module_path_from_file(
    package_dir: &Path,
    file_path: &Path,
) -> Result<Vec<String>, CheckFailure> {
    let source_root = package_dir.join("src");
    let relative = file_path.strip_prefix(&source_root).map_err(|error| {
        structural_error(format!(
            "정의 파일이 패키지 src 밖에 있다: {} ({error})",
            file_path.display()
        ))
    })?;
    let mut segments: Vec<String> = relative
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    let Some(last) = segments.pop() else {
        return Err(structural_error("정의 파일 경로가 비어 있다"));
    };
    let stem = last
        .strip_suffix(".rs")
        .ok_or_else(|| structural_error(format!("정의 파일이 .rs 파일이 아니다: {last}")))?;
    if stem != "lib" && stem != "mod" {
        segments.push(String::from(stem));
    }
    Ok(segments)
}

fn canonical_item_paths(
    defining_pkg: &MemberPackage,
    defining_path: &Path,
    rules: &IdentityConfirmationRules,
) -> Result<(String, String), CheckFailure> {
    let crate_ident = defining_pkg.name.as_str().replace('-', "_");
    let module_segments = module_path_from_file(&defining_pkg.directory, defining_path)?;
    let prefix = if module_segments.is_empty() {
        crate_ident
    } else {
        format!("{crate_ident}::{}", module_segments.join("::"))
    };
    let issuer = format!(
        "{prefix}::{}::{}",
        rules.confirmation_type, rules.issuer_function
    );
    let seal = format!("{prefix}::{}", rules.seal_type);
    Ok((issuer, seal))
}

fn load_linux_excluded_crates(root: &Path) -> Result<BTreeSet<CrateName>, CheckFailure> {
    if std::env::consts::OS != "linux" {
        return Ok(BTreeSet::new());
    }
    let path = root.join(LINUX_EXCLUSIONS_PATH);
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let text = fs::read_to_string(&path).map_err(|error| io_failure(&path, error))?;
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|error| structural_error(format!("{LINUX_EXCLUSIONS_PATH} 파싱 실패: {error}")))?;
    let entries = parsed
        .get("crates")
        .and_then(Value::as_array)
        .ok_or_else(|| structural_error(format!("{LINUX_EXCLUSIONS_PATH}: crates 배열이 없다")))?;
    let mut excluded = BTreeSet::new();
    for entry in entries {
        let name = entry.get("name").and_then(Value::as_str).ok_or_else(|| {
            structural_error(format!(
                "{LINUX_EXCLUSIONS_PATH}: crates 항목에 name이 없다"
            ))
        })?;
        excluded.insert(CrateName::new(name));
    }
    Ok(excluded)
}

fn parse_rust_file(path: &Path) -> Result<syn::File, CheckFailure> {
    let text = fs::read_to_string(path).map_err(|error| io_failure(path, error))?;
    syn::parse_file(&text)
        .map_err(|error| structural_error(format!("{} 파싱 실패: {error}", path.display())))
}

fn collect_defining_sites(
    defining_path: &Path,
    rules: &IdentityConfirmationRules,
) -> Result<DefiningSites, CheckFailure> {
    let parsed = parse_rust_file(defining_path)?;
    let mut test_spans = Vec::new();
    let mut seal_ranges = Vec::new();
    let mut confirmation_ranges = Vec::new();
    let mut has_test_issuer = false;
    for item in &parsed.items {
        match item {
            Item::Struct(structure) => {
                let name = structure.ident.to_string();
                let clean = name.strip_prefix("r#").unwrap_or(&name);
                if clean == rules.seal_type {
                    seal_ranges.push(item_line_range(&structure.attrs, structure.span()));
                } else if clean == rules.confirmation_type {
                    confirmation_ranges.push(item_line_range(&structure.attrs, structure.span()));
                }
            }
            Item::Mod(module) if is_cfg_test_mod(module) => {
                test_spans.push(item_span_range(&module.attrs, module.span()));
                let mut collector = DirectIssuerCallCollector {
                    rules,
                    spans: Vec::new(),
                };
                collector.visit_item_mod(module);
                if !collector.spans.is_empty() {
                    has_test_issuer = true;
                }
            }
            Item::Const(_)
            | Item::Enum(_)
            | Item::ExternCrate(_)
            | Item::Fn(_)
            | Item::ForeignMod(_)
            | Item::Impl(_)
            | Item::Macro(_)
            | Item::Mod(_)
            | Item::Static(_)
            | Item::Trait(_)
            | Item::TraitAlias(_)
            | Item::Type(_)
            | Item::Union(_)
            | Item::Use(_)
            | Item::Verbatim(_)
            | _ => {}
        }
    }
    let canary = if has_test_issuer {
        ExpectedCanary::Expected
    } else {
        ExpectedCanary::NotExpected
    };
    Ok(DefiningSites {
        test_spans,
        seal_ranges,
        confirmation_ranges,
        canary,
    })
}

fn collect_test_mod_ranges(path: &Path) -> Result<Vec<LineRange>, CheckFailure> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let parsed = parse_rust_file(path)?;
    let mut ranges = Vec::new();
    for item in &parsed.items {
        if let Item::Mod(module) = item
            && is_cfg_test_mod(module)
        {
            ranges.push(item_line_range(&module.attrs, module.span()));
        }
    }
    Ok(ranges)
}

fn collect_allowed_sites(
    workspace: &Workspace,
    defining_pkg: &MemberPackage,
    rules: &IdentityConfirmationRules,
    target_packages: &BTreeSet<CrateName>,
) -> Result<AllowedStructuralSites, CheckFailure> {
    let root = &workspace.root;
    let defining_path = root.join(&rules.defining_file);
    let defining = collect_defining_sites(&defining_path, rules)?;
    let defining_test_file_ranges = collect_test_mod_ranges(&root.join(&rules.defining_test_file))?;
    let mut allowed_direct_call_spans = BTreeMap::new();
    let mut expected_issuer_packages = BTreeSet::new();
    if defining.canary == ExpectedCanary::Expected && target_packages.contains(&defining_pkg.name) {
        expected_issuer_packages.insert(defining_pkg.name.clone());
    }
    for relative in &rules.issuance_allowed_files {
        let full_path = root.join(relative);
        if !full_path.exists() {
            continue;
        }
        let parsed = parse_rust_file(&full_path)?;
        let mut collector = DirectIssuerCallCollector {
            rules,
            spans: Vec::new(),
        };
        collector.visit_file(&parsed);
        if !collector.spans.is_empty()
            && let Some(pkg) = defining_package_for_file(workspace, relative)
            && target_packages.contains(&pkg.name)
        {
            expected_issuer_packages.insert(pkg.name.clone());
        }
        allowed_direct_call_spans.insert(relative.clone(), collector.spans);
    }
    Ok(AllowedStructuralSites {
        defining_file: rules.defining_file.clone(),
        defining_test_file: rules.defining_test_file.clone(),
        defining_test_spans: defining.test_spans,
        seal_struct_ranges: defining.seal_ranges,
        confirmation_struct_ranges: defining.confirmation_ranges,
        defining_test_file_ranges,
        allowed_direct_call_spans,
        expected_issuer_packages,
    })
}

fn write_if_changed(path: &Path, content: &str) -> Result<(), CheckFailure> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(());
    }
    fs::write(path, content).map_err(|error| io_failure(path, error))
}

fn prepare_clippy_config(
    structural_target: &Path,
    issuer_path: &str,
    seal_path: &str,
) -> Result<PathBuf, CheckFailure> {
    let conf_dir = structural_target.join(CONF_SUBDIR);
    fs::create_dir_all(&conf_dir).map_err(|error| io_failure(&conf_dir, error))?;
    let issuer_lit = toml::Value::String(String::from(issuer_path));
    let issuer_reason = toml::Value::String(String::from(ISSUER_DISALLOWED_REASON));
    let seal_lit = toml::Value::String(String::from(seal_path));
    let seal_reason = toml::Value::String(String::from(SEAL_DISALLOWED_REASON));
    let content = format!(
        "disallowed-methods = [\n  {{ path = {issuer_lit}, reason = {issuer_reason} }},\n]\ndisallowed-types = [\n  {{ path = {seal_lit}, reason = {seal_reason} }},\n]\n"
    );
    write_if_changed(&conf_dir.join("clippy.toml"), &content)?;
    Ok(conf_dir)
}

fn isolate_cargo_environment(
    command: &mut Command,
    conf_dir: &Path,
    extra_inherited: &[(&str, &str)],
) {
    for (key, value) in extra_inherited {
        command.env(key, value);
    }
    for key in CLEARED_ENV_VARS {
        command.env_remove(key);
    }
    command.env("RUSTFLAGS", "");
    command.env("RUSTC_WRAPPER", "");
    command.env("RUSTC_WORKSPACE_WRAPPER", "");
    command.env("CLIPPY_CONF_DIR", conf_dir);
}

fn push_clippy_config_overrides(command: &mut Command, conf_dir: &Path) {
    let conf_lit = toml::Value::String(conf_dir.to_string_lossy().into_owned());
    let overrides = [
        String::from("build.rustc-wrapper=\"\""),
        String::from("build.rustflags=[]"),
        format!("env.CLIPPY_CONF_DIR.value={conf_lit}"),
        String::from("env.CLIPPY_CONF_DIR.force=true"),
        String::from("env.RUSTFLAGS.value=\"\""),
        String::from("env.RUSTFLAGS.force=true"),
        String::from("env.RUSTC_WRAPPER.value=\"\""),
        String::from("env.RUSTC_WRAPPER.force=true"),
    ];
    for setting in overrides {
        command.arg("--config").arg(setting);
    }
}

struct ClippyInvocation<'a> {
    target_packages: &'a BTreeSet<CrateName>,
    structural_target: &'a Path,
    conf_dir: &'a Path,
    extra_inherited: &'a [(&'a str, &'a str)],
}

fn run_structural_clippy(
    workspace: &Workspace,
    invocation: &ClippyInvocation<'_>,
) -> Result<Output, CheckFailure> {
    let mut command = Command::new("cargo-clippy");
    command.current_dir(&workspace.root);
    isolate_cargo_environment(
        &mut command,
        invocation.conf_dir,
        invocation.extra_inherited,
    );
    if invocation
        .target_packages
        .contains(&CrateName::new("src-tauri"))
    {
        let empty_dist = invocation.structural_target.join(EMPTY_DIST_SUBDIR);
        fs::create_dir_all(&empty_dist).map_err(|error| io_failure(&empty_dist, error))?;
        let tauri_config = json!({
            "build": {
                "frontendDist": empty_dist.to_string_lossy().into_owned(),
            }
        });
        command.env("TAURI_CONFIG", tauri_config.to_string());
    }
    command.arg("clippy");
    push_clippy_config_overrides(&mut command, invocation.conf_dir);
    command
        .args([
            "--locked",
            "--offline",
            "--all-targets",
            "--all-features",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(workspace.root.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(invocation.structural_target);
    for pkg in invocation.target_packages {
        command.arg("-p").arg(pkg.as_str());
    }
    command.args([
        "--",
        "--cap-lints=warn",
        "-A",
        "warnings",
        "--force-warn",
        "clippy::disallowed_methods",
        "--force-warn",
        "clippy::disallowed_types",
    ]);
    command
        .output()
        .map_err(|error| structural_error(format!("cargo clippy 실행 실패: {error}")))
}

fn normalize_relative(canonical_root: &Path, raw_file: &str) -> SpanLocation {
    let raw_path = Path::new(raw_file);
    let candidate = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        canonical_root.join(raw_path)
    };
    if let Ok(canonical) = fs::canonicalize(&candidate) {
        return match canonical.strip_prefix(canonical_root) {
            Ok(stripped) => SpanLocation::Workspace(
                stripped
                    .iter()
                    .map(|part| part.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/"),
            ),
            Err(_) => SpanLocation::External(String::from(raw_file)),
        };
    }
    if raw_path.is_absolute() {
        match raw_path.strip_prefix(canonical_root) {
            Ok(stripped) => SpanLocation::Workspace(
                stripped
                    .iter()
                    .map(|part| part.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/"),
            ),
            Err(_) => SpanLocation::External(String::from(raw_file)),
        }
    } else {
        SpanLocation::Workspace(raw_file.replace('\\', "/"))
    }
}

fn span_lines(span: &Value) -> Option<(usize, usize)> {
    let start_raw = span.get("line_start").and_then(Value::as_u64)?;
    let end_raw = span.get("line_end").and_then(Value::as_u64)?;
    let start = usize::try_from(start_raw).ok()?;
    let end = usize::try_from(end_raw).ok()?;
    Some((start, end))
}

fn span_range(span: &Value) -> Option<SpanRange> {
    let (start_line, end_line) = span_lines(span)?;
    let start_col_raw = span.get("column_start").and_then(Value::as_u64)?;
    let end_col_raw = span.get("column_end").and_then(Value::as_u64)?;
    let start_column = usize::try_from(start_col_raw).ok()?;
    let end_column = usize::try_from(end_col_raw).ok()?;
    Some(SpanRange {
        start_line,
        start_column,
        end_line,
        end_column,
    })
}

fn outermost_expansion_span(span: &Value) -> &Value {
    let mut current = span;
    while let Some(next) = current.get("expansion").and_then(|exp| exp.get("span")) {
        current = next;
    }
    current
}

fn subject_for_span(canonical_root: &Path, span: &Value) -> String {
    let mut best_inside = None;
    let mut fallback = String::from("(structural:unknown-file)");
    let mut current = Some(span);
    while let Some(node) = current {
        if let Some(raw_file) = node.get("file_name").and_then(Value::as_str) {
            match normalize_relative(canonical_root, raw_file) {
                SpanLocation::Workspace(normalized) => {
                    best_inside = Some(normalized.clone());
                    fallback = normalized;
                }
                SpanLocation::External(normalized) => {
                    fallback = normalized;
                }
            }
        }
        current = node.get("expansion").and_then(|exp| exp.get("span"));
    }
    best_inside.unwrap_or(fallback)
}

fn ranges_contain(ranges: &[LineRange], line_start: usize, line_end: usize) -> bool {
    ranges
        .iter()
        .any(|range| range.contains(line_start, line_end))
}

fn span_ranges_contain(ranges: &[SpanRange], actual: SpanRange) -> bool {
    ranges.iter().any(|range| range.contains(actual))
}

fn span_has_expansion(span: &Value) -> bool {
    span.get("expansion").is_some_and(|value| !value.is_null())
}

struct MessageContext {
    package_name: Option<CrateName>,
    package_dir: Option<PathBuf>,
    is_lib_target: TargetKindClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKindClass {
    Lib,
    Other,
}

fn message_context(entry: &Value, dir_to_package: &BTreeMap<PathBuf, CrateName>) -> MessageContext {
    let package_dir = entry
        .get("manifest_path")
        .and_then(Value::as_str)
        .and_then(|manifest| Path::new(manifest).parent())
        .map(canonical_or_original);
    let package_name = package_dir
        .as_ref()
        .and_then(|dir| dir_to_package.get(dir))
        .cloned();
    let has_lib = entry
        .get("target")
        .and_then(|target| target.get("kind"))
        .and_then(Value::as_array)
        .is_some_and(|kinds| {
            kinds
                .iter()
                .any(|kind| kind.as_str().is_some_and(|text| text == "lib"))
        });
    let is_lib_target = if has_lib {
        TargetKindClass::Lib
    } else {
        TargetKindClass::Other
    };
    MessageContext {
        package_name,
        package_dir,
        is_lib_target,
    }
}

fn span_belongs_to_package(
    canonical_root: &Path,
    relative: &str,
    context: &MessageContext,
) -> bool {
    if context.is_lib_target != TargetKindClass::Lib {
        return false;
    }
    let Some(pkg_dir) = &context.package_dir else {
        return false;
    };
    canonical_or_original(&canonical_root.join(relative)).starts_with(pkg_dir)
}

fn is_allowed_issuer_span(
    canonical_root: &Path,
    sites: &AllowedStructuralSites,
    context: &MessageContext,
    span: &Value,
) -> bool {
    if span_has_expansion(span) {
        return false;
    }
    let Some(raw_file) = span.get("file_name").and_then(Value::as_str) else {
        return false;
    };
    let SpanLocation::Workspace(relative) = normalize_relative(canonical_root, raw_file) else {
        return false;
    };
    if !span_belongs_to_package(canonical_root, &relative, context) {
        return false;
    }
    let Some(actual_span) = span_range(span) else {
        return false;
    };
    if relative == sites.defining_file {
        return span_ranges_contain(&sites.defining_test_spans, actual_span);
    }
    sites
        .allowed_direct_call_spans
        .get(&relative)
        .is_some_and(|ranges| span_ranges_contain(ranges, actual_span))
}

fn is_allowed_seal_span(
    canonical_root: &Path,
    sites: &AllowedStructuralSites,
    context: &MessageContext,
    span: &Value,
) -> bool {
    let Some(raw_file) = span.get("file_name").and_then(Value::as_str) else {
        return false;
    };
    let SpanLocation::Workspace(relative) = normalize_relative(canonical_root, raw_file) else {
        return false;
    };
    if !span_belongs_to_package(canonical_root, &relative, context) {
        return false;
    }
    let Some(actual_span) = span_range(span) else {
        return false;
    };
    let (line_start, line_end) = (actual_span.start_line, actual_span.end_line);
    if span_has_expansion(span) {
        let outer = outermost_expansion_span(span);
        let Some(outer_file) = outer.get("file_name").and_then(Value::as_str) else {
            return false;
        };
        let SpanLocation::Workspace(outer_rel) = normalize_relative(canonical_root, outer_file)
        else {
            return false;
        };
        let Some((outer_start, outer_end)) = span_lines(outer) else {
            return false;
        };
        return outer_rel == sites.defining_file
            && relative == sites.defining_file
            && ranges_contain(&sites.seal_struct_ranges, outer_start, outer_end)
            && ranges_contain(&sites.seal_struct_ranges, line_start, line_end);
    }
    if relative == sites.defining_file {
        return ranges_contain(&sites.seal_struct_ranges, line_start, line_end)
            || ranges_contain(&sites.confirmation_struct_ranges, line_start, line_end)
            || span_ranges_contain(&sites.defining_test_spans, actual_span);
    }
    relative == sites.defining_test_file
        && ranges_contain(&sites.defining_test_file_ranges, line_start, line_end)
}

fn format_violation_subject(
    canonical_root: &Path,
    context: &MessageContext,
    span: &Value,
) -> String {
    let subject = subject_for_span(canonical_root, span);
    let inside_pkg = context
        .package_dir
        .as_ref()
        .is_some_and(|dir| canonical_or_original(&canonical_root.join(&subject)).starts_with(dir));
    if !inside_pkg && let Some(pkg) = &context.package_name {
        return format!("{pkg}:{subject}");
    }
    subject
}

struct EvaluationContext<'a> {
    canonical_root: &'a Path,
    sites: &'a AllowedStructuralSites,
    message: &'a MessageContext,
}

fn evaluate_diagnostic(
    eval: &EvaluationContext<'_>,
    kind: DiagnosticKind,
    message: &Value,
    findings: &mut BTreeSet<Finding>,
) {
    let spans = message
        .get("spans")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let primary: Vec<&Value> = spans
        .iter()
        .filter(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
        .collect();
    let inspected: Vec<&Value> = if primary.is_empty() {
        spans.iter().collect()
    } else {
        primary
    };
    if inspected.is_empty() {
        findings.insert(Finding::new(
            Rule::IdentityConfirmationIssuance,
            String::from("(structural:missing-span)"),
        ));
        return;
    }
    for span in inspected {
        let allowed = match kind {
            DiagnosticKind::IssuerMethod => {
                is_allowed_issuer_span(eval.canonical_root, eval.sites, eval.message, span)
            }
            DiagnosticKind::SealType => {
                is_allowed_seal_span(eval.canonical_root, eval.sites, eval.message, span)
            }
        };
        if !allowed {
            let subject = format_violation_subject(eval.canonical_root, eval.message, span);
            findings.insert(Finding::new(Rule::IdentityConfirmationIssuance, subject));
        }
    }
}

fn record_artifact(
    entry: &Value,
    dir_to_package: &BTreeMap<PathBuf, CrateName>,
    observed: &mut BTreeSet<CrateName>,
) {
    let Some(manifest_str) = entry.get("manifest_path").and_then(Value::as_str) else {
        return;
    };
    let Some(parent) = Path::new(manifest_str).parent() else {
        return;
    };
    let canonical = canonical_or_original(parent);
    if let Some(pkg_name) = dir_to_package.get(&canonical) {
        observed.insert(pkg_name.clone());
    }
}

fn handle_compiler_message(
    eval: &EvaluationContext<'_>,
    message: &Value,
    parsed: &mut ParsedOutput,
) {
    let level = message.get("level").and_then(Value::as_str);
    if level == Some("error") {
        let text = message
            .get("rendered")
            .or_else(|| message.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("compiler error");
        parsed.compiler_errors.push(String::from(text));
        return;
    }
    let code = message
        .get("code")
        .and_then(|obj| obj.get("code"))
        .and_then(Value::as_str);
    match code {
        Some("clippy::disallowed_methods") => {
            if let Some(pkg) = &eval.message.package_name {
                parsed.seen_issuer_packages.insert(pkg.clone());
            }
            evaluate_diagnostic(
                eval,
                DiagnosticKind::IssuerMethod,
                message,
                &mut parsed.findings,
            );
        }
        Some("clippy::disallowed_types") => {
            parsed.seen_seal = parsed.seen_seal.saturating_add(1);
            evaluate_diagnostic(
                eval,
                DiagnosticKind::SealType,
                message,
                &mut parsed.findings,
            );
        }
        Some(_) | None => {}
    }
}

fn parse_clippy_jsonl(
    stdout: &[u8],
    canonical_root: &Path,
    dir_to_package: &BTreeMap<PathBuf, CrateName>,
    sites: &AllowedStructuralSites,
) -> Result<ParsedOutput, CheckFailure> {
    let mut parsed = ParsedOutput {
        observed_packages: BTreeSet::new(),
        compiler_errors: Vec::new(),
        seen_issuer_packages: BTreeSet::new(),
        seen_seal: 0,
        findings: BTreeSet::new(),
    };
    let text = String::from_utf8_lossy(stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(trimmed)
            .map_err(|error| structural_error(format!("clippy JSON 파싱 실패: {error}")))?;
        match entry.get("reason").and_then(Value::as_str) {
            Some("compiler-artifact") => {
                record_artifact(&entry, dir_to_package, &mut parsed.observed_packages);
            }
            Some("compiler-message") => {
                if let Some(message) = entry.get("message") {
                    let msg_ctx = message_context(&entry, dir_to_package);
                    let eval = EvaluationContext {
                        canonical_root,
                        sites,
                        message: &msg_ctx,
                    };
                    handle_compiler_message(&eval, message, &mut parsed);
                }
            }
            Some(_) | None => {}
        }
    }
    Ok(parsed)
}

fn validate_parsed_output(
    status: CommandStatus,
    stderr: &[u8],
    target_packages: &BTreeSet<CrateName>,
    sites: &AllowedStructuralSites,
    parsed: ParsedOutput,
) -> Result<BTreeSet<Finding>, CheckFailure> {
    if status == CommandStatus::Failed || !parsed.compiler_errors.is_empty() {
        let detail = if parsed.compiler_errors.is_empty() {
            String::from_utf8_lossy(stderr).trim().to_owned()
        } else {
            parsed.compiler_errors.join("\n")
        };
        return Err(structural_error(format!("clippy 실패:\n{detail}")));
    }
    let missing: Vec<&str> = target_packages
        .difference(&parsed.observed_packages)
        .map(CrateName::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(structural_error(format!(
            "아티팩트를 관측하지 못한 패키지: {}",
            missing.join(", ")
        )));
    }
    if parsed.seen_seal == 0 {
        return Err(structural_error(
            "봉인 타입 기준 진단을 관측하지 못했다(구조적 검사 비활성 의심)",
        ));
    }
    let missing_issuer: Vec<&str> = sites
        .expected_issuer_packages
        .difference(&parsed.seen_issuer_packages)
        .map(CrateName::as_str)
        .collect();
    if !missing_issuer.is_empty() {
        return Err(structural_error(format!(
            "발급 함수 기준 진단을 관측하지 못한 패키지(구조적 검사 비활성 의심): {}",
            missing_issuer.join(", ")
        )));
    }
    Ok(parsed.findings)
}

pub(crate) fn has_identity_confirmation_surface(
    workspace: &Workspace,
    rules: &IdentityConfirmationRules,
) -> bool {
    workspace
        .packages
        .iter()
        .any(|package| package.name.as_str() == "xtask")
        || workspace
            .root
            .join("harness/claude-hooks/hashes.json")
            .exists()
        || workspace.root.join(&rules.defining_file).exists()
        || workspace.root.join(&rules.defining_test_file).exists()
        || rules
            .issuance_allowed_files
            .iter()
            .any(|relative| workspace.root.join(relative).exists())
}

pub(crate) fn check_structural_with_env(
    workspace: &Workspace,
    rules: &Rules,
    extra_inherited: &[(&str, &str)],
) -> Result<BTreeSet<Finding>, CheckFailure> {
    let id_rules = &rules.source.identity_confirmation;
    let Some(defining_pkg) = defining_package_for_file(workspace, &id_rules.defining_file) else {
        return Err(structural_error(format!(
            "정의 파일의 소속 패키지를 찾지 못했다: {}",
            id_rules.defining_file
        )));
    };
    let defining_path = workspace.root.join(&id_rules.defining_file);
    if !defining_path.exists() {
        return Err(structural_error(format!(
            "정의 파일이 없다: {}",
            id_rules.defining_file
        )));
    }
    let excluded = load_linux_excluded_crates(&workspace.root)?;
    let target_packages: BTreeSet<CrateName> = workspace
        .graph
        .members_reaching(&defining_pkg.name)
        .into_iter()
        .filter(|name| !excluded.contains(name))
        .collect();
    if target_packages.is_empty() {
        return Err(structural_error("구조적 검사 대상 패키지가 비어 있다"));
    }
    let (issuer_path, seal_path) = canonical_item_paths(defining_pkg, &defining_path, id_rules)?;
    let sites = collect_allowed_sites(workspace, defining_pkg, id_rules, &target_packages)?;
    let structural_target = workspace.target_directory.join(STRUCTURAL_CACHE_DIR);
    let conf_dir = prepare_clippy_config(&structural_target, &issuer_path, &seal_path)?;
    let invocation = ClippyInvocation {
        target_packages: &target_packages,
        structural_target: &structural_target,
        conf_dir: &conf_dir,
        extra_inherited,
    };
    let output = run_structural_clippy(workspace, &invocation)?;
    let canonical_root = canonical_or_original(&workspace.root);
    let dir_to_package: BTreeMap<PathBuf, CrateName> = workspace
        .packages
        .iter()
        .map(|package| {
            (
                canonical_or_original(&package.directory),
                package.name.clone(),
            )
        })
        .collect();
    let parsed = parse_clippy_jsonl(&output.stdout, &canonical_root, &dir_to_package, &sites)?;
    let status = if output.status.success() {
        CommandStatus::Succeeded
    } else {
        CommandStatus::Failed
    };
    validate_parsed_output(status, &output.stderr, &target_packages, &sites, parsed)
}

pub(crate) fn check_structural(
    workspace: &Workspace,
    rules: &Rules,
) -> Result<BTreeSet<Finding>, CheckFailure> {
    check_structural_with_env(workspace, rules, &[])
}
