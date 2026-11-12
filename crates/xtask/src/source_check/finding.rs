use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExpectProblem {
    Inner,
    NoReason,
    MultipleLints,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    Comment,
    Unparsable,
    Attribute(String),
    Expect(ExpectProblem),
    CfgNotTest,
    CfgTestOutsideModule,
    CfgMacro,
    Macro(String),
    DomainFloat(String),
    ClockOrEnvironment(String),
    LintsNotInherited,
    ManifestTable(String),
    BuildScript,
    ProcMacro,
    NestedConfig(String),
    CargoConfig,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comment => f.write_str("comment"),
            Self::Unparsable => f.write_str("unparsable"),
            Self::Attribute(name) => write!(f, "attribute:{name}"),
            Self::Expect(ExpectProblem::Inner) => f.write_str("expect:inner"),
            Self::Expect(ExpectProblem::NoReason) => f.write_str("expect:no-reason"),
            Self::Expect(ExpectProblem::MultipleLints) => f.write_str("expect:multiple-lints"),
            Self::Expect(ExpectProblem::Malformed) => f.write_str("expect:malformed"),
            Self::CfgNotTest => f.write_str("cfg:not-test"),
            Self::CfgTestOutsideModule => f.write_str("cfg:test-outside-module"),
            Self::CfgMacro => f.write_str("cfg:macro"),
            Self::Macro(name) => write!(f, "macro:{name}"),
            Self::DomainFloat(name) => write!(f, "domain-float:{name}"),
            Self::ClockOrEnvironment(name) => write!(f, "clock-env:{name}"),
            Self::LintsNotInherited => f.write_str("manifest:lints-not-inherited"),
            Self::ManifestTable(name) => write!(f, "manifest:table:{name}"),
            Self::BuildScript => f.write_str("file:build-script"),
            Self::ProcMacro => f.write_str("file:proc-macro"),
            Self::NestedConfig(name) => write!(f, "file:nested-config:{name}"),
            Self::CargoConfig => f.write_str("file:cargo-config"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Location {
    WholeFile,
    Line(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    pub(crate) path: String,
    pub(crate) location: Location,
    pub(crate) rule: Rule,
}

impl Finding {
    pub(crate) fn whole_file(path: &str, rule: Rule) -> Self {
        Self {
            path: String::from(path),
            location: Location::WholeFile,
            rule,
        }
    }

    pub(crate) fn at_line(path: &str, line: usize, rule: Rule) -> Self {
        Self {
            path: String::from(path),
            location: Location::Line(line),
            rule,
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Location::WholeFile => write!(f, "{}: {}", self.path, self.rule),
            Location::Line(line) => write!(f, "{}:{line}: {}", self.path, self.rule),
        }
    }
}
