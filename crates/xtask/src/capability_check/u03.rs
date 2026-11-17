use std::collections::BTreeSet;
use std::path::Path;

use crate::check_support::Finding;

use super::{CONFIG_FILE, Findings, Json, Rule, read_json};

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src ipc:; worker-src 'none'; font-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";
const MAIN_FILE: &str = "capabilities/main.json";

fn config_findings(config: &Json, findings: &mut Findings) {
    let security = config.get("app").and_then(|app| app.get("security"));
    if security
        .and_then(|value| value.get("csp"))
        .and_then(Json::as_str)
        != Some(CSP)
        || security.and_then(|value| value.get("devCsp")).is_some()
        || security
            .and_then(|value| value.get("dangerousDisableAssetCspModification"))
            .is_some()
    {
        findings.add(CONFIG_FILE, Rule::U03Csp);
    }
    if security.and_then(|value| value.get("freezePrototype")) != Some(&Json::True) {
        findings.add(CONFIG_FILE, Rule::U03FreezePrototype);
    }
    let build = config.get("build");
    if build
        .and_then(|value| value.get("frontendDist"))
        .and_then(Json::as_str)
        != Some("../dist")
        || build.and_then(|value| value.get("devUrl")).is_some()
    {
        findings.add(CONFIG_FILE, Rule::U03Frontend);
    }
    let windows = config
        .get("app")
        .and_then(|app| app.get("windows"))
        .and_then(Json::as_array);
    if !windows.is_some_and(|listed| {
        if let [window] = listed {
            window.get("label").and_then(Json::as_str) == Some("main")
                && window.get("create") == Some(&Json::False)
                && window.get("url").is_none()
        } else {
            false
        }
    }) {
        findings.add(CONFIG_FILE, Rule::U03Window);
    }
}

fn event_findings(capability: &Json, findings: &mut Findings) {
    let permissions = capability.get("permissions").and_then(Json::as_array);
    let expected = [
        Json::Text(String::from("commands")),
        Json::Text(String::from("core:event:allow-listen")),
        Json::Text(String::from("core:event:allow-unlisten")),
    ];
    if permissions != Some(expected.as_slice()) {
        findings.add(MAIN_FILE, Rule::U03EventPermissions);
    }
}

pub(super) fn check_u03_directory(directory: &Path) -> BTreeSet<Finding<Rule>> {
    let mut findings = Findings {
        items: BTreeSet::new(),
    };
    if let Some(config) = read_json(directory, CONFIG_FILE, &mut findings) {
        config_findings(&config, &mut findings);
    } else {
        findings.add(CONFIG_FILE, Rule::U03Csp);
    }
    if let Some(capability) = read_json(directory, MAIN_FILE, &mut findings) {
        event_findings(&capability, &mut findings);
    } else {
        findings.add(MAIN_FILE, Rule::U03EventPermissions);
    }
    findings.items
}
