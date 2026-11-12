use std::collections::{BTreeMap, BTreeSet};

use super::rules::{
    ExternalLocation, FeatureAllowlist, FeatureLocation, GraphRules, LocationScope, Restriction,
    TransitiveBan,
};
use super::{CrateName, Finding, Rule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DepKind {
    Normal,
    Build,
    Dev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultFeatures {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Member,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NodeKey(String);

impl NodeKey {
    pub(crate) fn new(key: &str) -> Self {
        Self(String::from(key))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub(crate) to: NodeKey,
    pub(crate) kind: DepKind,
}

#[derive(Debug, Clone)]
pub(crate) struct Declaration {
    pub(crate) dependency: CrateName,
    pub(crate) default_features: DefaultFeatures,
    pub(crate) features: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub(crate) name: CrateName,
    pub(crate) origin: Origin,
    pub(crate) edges: Vec<Edge>,
    pub(crate) declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CrateGraph {
    pub(crate) nodes: BTreeMap<NodeKey, Node>,
}

#[derive(Debug, Clone, Copy)]
enum FirstHop {
    AnyKind,
    LinkedOnly,
}

impl CrateGraph {
    fn members(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .values()
            .filter(|node| node.origin == Origin::Member)
    }

    fn targets<'graph>(
        &'graph self,
        node: &'graph Node,
    ) -> impl Iterator<Item = (&'graph Node, DepKind)> {
        node.edges
            .iter()
            .filter_map(|edge| self.nodes.get(&edge.to).map(|target| (target, edge.kind)))
    }

    fn reachable<'graph>(
        &'graph self,
        start: &'graph Node,
        first_hop: FirstHop,
    ) -> BTreeSet<&'graph NodeKey> {
        let mut seen: BTreeSet<&NodeKey> = BTreeSet::new();
        let mut pending: Vec<&NodeKey> = start
            .edges
            .iter()
            .filter(|edge| match (first_hop, edge.kind) {
                (FirstHop::AnyKind, _)
                | (FirstHop::LinkedOnly, DepKind::Normal | DepKind::Build) => true,
                (FirstHop::LinkedOnly, DepKind::Dev) => false,
            })
            .map(|edge| &edge.to)
            .collect();
        while let Some(key) = pending.pop() {
            if seen.insert(key)
                && let Some(node) = self.nodes.get(key)
            {
                pending.extend(
                    node.edges
                        .iter()
                        .filter(|edge| edge.kind != DepKind::Dev)
                        .map(|edge| &edge.to),
                );
            }
        }
        seen
    }

    fn reaches_member(&self, start: &Node, target: &CrateName, first_hop: FirstHop) -> Option<()> {
        self.reachable(start, first_hop)
            .into_iter()
            .filter_map(|key| self.nodes.get(key))
            .find(|node| node.origin == Origin::Member && &node.name == target)
            .map(|_| ())
    }
}

fn arrow(from: &CrateName, to: &CrateName) -> String {
    format!("{from} -> {to}")
}

fn reach(from: &CrateName, to: &CrateName) -> String {
    format!("{from} => {to}")
}

fn unknown_crates(rules: &GraphRules, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    findings.extend(
        graph
            .members()
            .filter(|member| !rules.edges.contains_key(&member.name))
            .map(|member| Finding::new(Rule::UnknownCrate, member.name.to_string())),
    );
}

fn member_edges(rules: &GraphRules, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    let nothing = BTreeSet::new();
    for member in graph.members() {
        let linked = rules.edges.get(&member.name).unwrap_or(&nothing);
        let dev = rules
            .dev_edges
            .crates
            .get(&member.name)
            .unwrap_or(&rules.dev_edges.other_crates);
        for (target, kind) in graph.targets(member) {
            let (allowed, rule) = match kind {
                DepKind::Normal | DepKind::Build => (linked, Rule::AllowedEdge),
                DepKind::Dev => (dev, Rule::DevEdge),
            };
            if target.origin == Origin::Member && !allowed.contains(&target.name) {
                findings.insert(Finding::new(rule, arrow(&member.name, &target.name)));
            }
        }
    }
}

fn permits(restriction: &Restriction, dependent: &CrateName, kind: DepKind) -> Option<()> {
    let listed = restriction
        .dependents
        .iter()
        .any(|allowed| &allowed.dependent == dependent && allowed.kinds.contains(&kind));
    (listed || restriction.any_dependent_kinds.contains(&kind)).then_some(())
}

fn bans(ban: &TransitiveBan, name: &CrateName) -> Option<()> {
    match ban {
        TransitiveBan::Nothing => None,
        TransitiveBan::Listed(listed) => listed.contains(name).then_some(()),
        TransitiveBan::AllExcept(kept) => (!kept.contains(name)).then_some(()),
    }
}

fn restriction(restriction: &Restriction, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    for member in graph.members() {
        for (target, kind) in graph.targets(member) {
            if target.origin == Origin::Member
                && target.name == restriction.target
                && permits(restriction, &member.name, kind).is_none()
            {
                findings.insert(Finding::new(
                    Rule::DirectDependent,
                    arrow(&member.name, &target.name),
                ));
            }
        }
    }
    findings.extend(
        graph
            .members()
            .filter(|member| member.name != restriction.target)
            .filter(|member| bans(&restriction.transitive, &member.name).is_some())
            .filter(|member| {
                graph
                    .reaches_member(member, &restriction.target, FirstHop::AnyKind)
                    .is_some()
            })
            .map(|member| {
                Finding::new(
                    Rule::TransitiveDependency,
                    reach(&member.name, &restriction.target),
                )
            }),
    );
}

fn product_bin(rules: &GraphRules, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    let package = &rules.product_bin.package;
    for member in graph.members().filter(|member| &member.name == package) {
        findings.extend(
            graph
                .reachable(member, FirstHop::LinkedOnly)
                .into_iter()
                .filter_map(|key| graph.nodes.get(key))
                .filter(|node| rules.product_bin.forbidden.contains(&node.name))
                .map(|node| Finding::new(Rule::ProductBinGraph, reach(package, &node.name))),
        );
    }
}

fn location_matches(location: &ExternalLocation, external: &CrateName) -> Option<()> {
    let named = location.crates.contains(external);
    let prefixed = location
        .prefixes
        .iter()
        .any(|prefix| external.as_str().starts_with(prefix.as_str()));
    (named || prefixed).then_some(())
}

fn location_refuses(location: &ExternalLocation, dependent: &CrateName) -> Option<()> {
    match &location.scope {
        LocationScope::AllowedIn(allowed) => (!allowed.contains(dependent)).then_some(()),
        LocationScope::ForbiddenIn(forbidden) => forbidden.contains(dependent).then_some(()),
    }
}

fn external_locations(rules: &GraphRules, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    for member in graph.members() {
        for (target, _) in graph.targets(member) {
            let refused = rules.external_locations.iter().any(|location| {
                location_matches(location, &target.name).is_some()
                    && location_refuses(location, &member.name).is_some()
            });
            if target.origin == Origin::External && refused {
                findings.insert(Finding::new(
                    Rule::ExternalLocation,
                    arrow(&member.name, &target.name),
                ));
            }
        }
    }
}

fn external_allowlists(rules: &GraphRules, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    for allowlist in &rules.external_allowlists {
        for member in graph
            .members()
            .filter(|member| member.name == allowlist.dependent)
        {
            findings.extend(
                graph
                    .targets(member)
                    .filter(|(target, kind)| {
                        target.origin == Origin::External
                            && *kind != DepKind::Dev
                            && !allowlist.allowed.contains(&target.name)
                    })
                    .map(|(target, _)| {
                        Finding::new(Rule::ExternalAllowlist, arrow(&member.name, &target.name))
                    }),
            );
        }
    }
}

fn feature_subject(member: &CrateName, external: &CrateName, feature: &str) -> String {
    format!("{member} -> {external}[{feature}]")
}

fn feature_location(location: &FeatureLocation, member: &Node, findings: &mut BTreeSet<Finding>) {
    for declaration in member
        .declarations
        .iter()
        .filter(|declaration| declaration.dependency == location.external)
    {
        findings.extend(
            declaration
                .features
                .intersection(&location.features)
                .map(|feature| {
                    Finding::new(
                        Rule::ExternalFeature,
                        feature_subject(&member.name, &location.external, feature),
                    )
                }),
        );
    }
}

fn feature_locations(rules: &GraphRules, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    for location in &rules.feature_locations {
        for member in graph
            .members()
            .filter(|member| !location.allowed_in.contains(&member.name))
        {
            feature_location(location, member, findings);
        }
    }
}

fn feature_allowlist(
    allowlist: &FeatureAllowlist,
    member: &Node,
    findings: &mut BTreeSet<Finding>,
) {
    for declaration in member
        .declarations
        .iter()
        .filter(|declaration| declaration.dependency == allowlist.external)
    {
        if declaration.default_features == DefaultFeatures::On
            && allowlist.default_features == DefaultFeatures::Off
        {
            findings.insert(Finding::new(
                Rule::ExternalFeature,
                feature_subject(&member.name, &allowlist.external, "default"),
            ));
        }
        findings.extend(
            declaration
                .features
                .difference(&allowlist.features)
                .map(|feature| {
                    Finding::new(
                        Rule::ExternalFeature,
                        feature_subject(&member.name, &allowlist.external, feature),
                    )
                }),
        );
    }
}

fn feature_allowlists(rules: &GraphRules, graph: &CrateGraph, findings: &mut BTreeSet<Finding>) {
    for allowlist in &rules.feature_allowlists {
        for member in graph
            .members()
            .filter(|member| member.name == allowlist.dependent)
        {
            feature_allowlist(allowlist, member, findings);
        }
    }
}

pub(crate) fn check_graph(rules: &GraphRules, graph: &CrateGraph) -> BTreeSet<Finding> {
    let mut findings = BTreeSet::new();
    unknown_crates(rules, graph, &mut findings);
    member_edges(rules, graph, &mut findings);
    for restricted in &rules.restricted {
        restriction(restricted, graph, &mut findings);
    }
    product_bin(rules, graph, &mut findings);
    external_locations(rules, graph, &mut findings);
    external_allowlists(rules, graph, &mut findings);
    feature_locations(rules, graph, &mut findings);
    feature_allowlists(rules, graph, &mut findings);
    findings
}
