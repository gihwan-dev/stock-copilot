use std::collections::BTreeSet;

const COMPANION: (&str, &str, &str) = (
    "crates/application",
    "testkit-ports",
    "crates/testkit-ports",
);

const DECLARATIONS: [(&str, &str, Option<&str>); 9] = [
    ("crates/domain", "domain", None),
    (COMPANION.0, "application", Some(COMPANION.1)),
    ("crates/order-execution", "order-execution", None),
    ("crates/approval-command", "approval-command", None),
    ("crates/adapters/store-sqlite", "store-sqlite", None),
    ("crates/adapters/broker-toss", "broker-toss", None),
    (
        "crates/adapters/broker-toss-trade",
        "broker-toss-trade",
        None,
    ),
    ("crates/adapters/secrets-keychain", "secrets-keychain", None),
    ("crates/adapters/platform-macos", "platform-macos", None),
];

pub(super) fn package_of_file(path: &str) -> Result<&'static str, String> {
    DECLARATIONS
        .iter()
        .find(|(directory, _, _)| path.starts_with(&format!("{directory}/src/")))
        .map(|(_, package, _)| *package)
        .ok_or_else(|| format!("변이 테스트 소유 선언이 없다: {path}"))
}

pub(super) fn selected(globs: &[String]) -> Result<BTreeSet<String>, String> {
    let mut packages = BTreeSet::new();
    for glob in globs {
        let (_, package, companion) = DECLARATIONS
            .iter()
            .find(|(directory, _, _)| glob.starts_with(&format!("{directory}/src/")))
            .ok_or_else(|| format!("변이 테스트 소유 선언이 없다: {glob}"))?;
        packages.insert(String::from(*package));
        if let Some(name) = companion {
            packages.insert(String::from(*name));
        }
    }
    Ok(packages)
}

pub(super) fn configure(
    command: &mut std::process::Command,
    globs: &[String],
) -> Result<(), String> {
    let packages = selected(globs)?;
    if packages.contains("testkit-ports") {
        for package in packages {
            command.arg(format!("--cargo-arg=--package={package}"));
            command.arg("--test-package").arg(package);
        }
    }
    Ok(())
}

pub(super) fn companion_sources(globs: &[String], changed: &BTreeSet<String>) -> BTreeSet<String> {
    if globs
        .iter()
        .any(|glob| glob.starts_with(&format!("{}/src/", COMPANION.0)))
        && changed
            .iter()
            .any(|path| path.starts_with(&format!("{}/", COMPANION.2)))
    {
        BTreeSet::from([String::from(COMPANION.0)])
    } else {
        BTreeSet::new()
    }
}

#[cfg(test)]
mod tests {
    fn assert_configured_arguments(sources: &[&str], packages: &[&str]) {
        let mut command = std::process::Command::new("cargo");
        command.args(["mutants", "--cap-lints=true"]);
        super::configure(
            &mut command,
            &sources
                .iter()
                .map(|source| String::from(*source))
                .collect::<Vec<_>>(),
        )
        .expect("declared combination");
        let mut expected = vec![
            std::ffi::OsString::from("mutants"),
            std::ffi::OsString::from("--cap-lints=true"),
        ];
        for package in packages {
            expected.extend([
                std::ffi::OsString::from(format!("--cargo-arg=--package={package}")),
                std::ffi::OsString::from("--test-package"),
                std::ffi::OsString::from(*package),
            ]);
        }
        assert_eq!(
            command
                .get_args()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>(),
            expected
        );
    }

    fn assert_source_subset(mut sources: Vec<&str>, packages: &[&str]) {
        assert_configured_arguments(&sources, &[]);
        sources.push("crates/application/src/lib.rs");
        assert_configured_arguments(&sources, packages);
    }

    #[test]
    fn empty_and_single_sources_preserve_existing_arguments_and_exact_owners() {
        let domain = "crates/domain/src/lib.rs";
        let order = "crates/order-execution/src/lib.rs";
        let store = "crates/adapters/store-sqlite/src/lib.rs";
        for (sources, packages) in [
            (vec![], vec!["application", "testkit-ports"]),
            (vec![domain], vec!["application", "domain", "testkit-ports"]),
            (
                vec![order],
                vec!["application", "order-execution", "testkit-ports"],
            ),
            (
                vec![store],
                vec!["application", "store-sqlite", "testkit-ports"],
            ),
        ] {
            assert_source_subset(sources, &packages);
        }
    }

    #[test]
    fn multiple_sources_preserve_existing_arguments_and_exact_owners() {
        let domain = "crates/domain/src/lib.rs";
        let order = "crates/order-execution/src/lib.rs";
        let store = "crates/adapters/store-sqlite/src/lib.rs";
        for (sources, packages) in [
            (
                vec![domain, order],
                vec!["application", "domain", "order-execution", "testkit-ports"],
            ),
            (
                vec![domain, store],
                vec!["application", "domain", "store-sqlite", "testkit-ports"],
            ),
            (
                vec![order, store],
                vec![
                    "application",
                    "order-execution",
                    "store-sqlite",
                    "testkit-ports",
                ],
            ),
            (
                vec![domain, order, store],
                vec![
                    "application",
                    "domain",
                    "order-execution",
                    "store-sqlite",
                    "testkit-ports",
                ],
            ),
        ] {
            assert_source_subset(sources, &packages);
        }
    }

    #[test]
    fn combinations_without_application_preserve_default_arguments() {
        for sources in [
            vec![
                "crates/domain/src/lib.rs",
                "crates/order-execution/src/lib.rs",
            ],
            vec![
                "crates/domain/src/lib.rs",
                "crates/adapters/store-sqlite/src/lib.rs",
            ],
            vec![
                "crates/order-execution/src/lib.rs",
                "crates/adapters/store-sqlite/src/lib.rs",
            ],
            vec![
                "crates/domain/src/lib.rs",
                "crates/order-execution/src/lib.rs",
                "crates/adapters/store-sqlite/src/lib.rs",
            ],
        ] {
            let mut command = std::process::Command::new("cargo");
            super::configure(
                &mut command,
                &sources.into_iter().map(String::from).collect::<Vec<_>>(),
            )
            .expect("declared owners");
            assert_eq!(command.get_args().count(), 0);
        }
    }
    #[test]
    fn companion_notice_requires_the_source_scope_and_exact_companion_directory() {
        let globs = [String::from("crates/application/src/**/*.rs")];
        let changed = std::collections::BTreeSet::from([String::from(
            "crates/testkit-ports/tests/submission.rs",
        )]);
        assert_eq!(
            super::companion_sources(&globs, &changed),
            std::collections::BTreeSet::from([String::from("crates/application")])
        );
        assert_eq!(
            super::companion_sources(&[], &changed),
            std::collections::BTreeSet::new()
        );
        assert_eq!(
            super::companion_sources(&globs, &std::collections::BTreeSet::new()),
            std::collections::BTreeSet::new()
        );
        for path in [
            "crates/testkit-ports-other/tests/test.rs",
            "other/crates/testkit-ports/tests/test.rs",
            "crates/domain/tests/test.rs",
        ] {
            assert_eq!(
                super::companion_sources(
                    &globs,
                    &std::collections::BTreeSet::from([String::from(path)])
                ),
                std::collections::BTreeSet::new()
            );
        }
        for glob in [
            "crates/domain/src/**/*.rs",
            "crates/application/src-other/**/*.rs",
            "other/crates/application/src/**/*.rs",
        ] {
            assert_eq!(
                super::companion_sources(&[String::from(glob)], &changed),
                std::collections::BTreeSet::new()
            );
        }
    }
    #[test]
    fn domain_keeps_default_arguments_and_application_adds_both_packages() {
        let mut domain = std::process::Command::new("cargo");
        super::configure(&mut domain, &[String::from("crates/domain/src/**/*.rs")])
            .expect("domain");
        assert_eq!(domain.get_args().count(), 0);
        let mut application = std::process::Command::new("cargo");
        super::configure(
            &mut application,
            &[String::from("crates/application/src/**/*.rs")],
        )
        .expect("application");
        assert_eq!(
            application.get_args().collect::<Vec<_>>(),
            [
                "--cargo-arg=--package=application",
                "--test-package",
                "application",
                "--cargo-arg=--package=testkit-ports",
                "--test-package",
                "testkit-ports"
            ]
        );
    }
    #[test]
    fn declared_test_owners_are_exact_for_files_and_globs() {
        for (directory, expected) in [
            ("crates/domain", vec!["domain"]),
            ("crates/application", vec!["application", "testkit-ports"]),
            ("crates/order-execution", vec!["order-execution"]),
            ("crates/adapters/store-sqlite", vec!["store-sqlite"]),
        ] {
            for file in ["lib.rs", "nested/use_case.rs", "**/*.rs"] {
                assert_eq!(
                    super::selected(&[format!("{directory}/src/{file}")]).expect("declaration"),
                    expected.iter().map(|name| String::from(*name)).collect()
                );
            }
        }
        assert_eq!(
            super::selected(&[]).expect("empty"),
            std::collections::BTreeSet::new()
        );
        assert_eq!(
            super::selected(
                &[
                    "crates/application/src/lib.rs",
                    "crates/domain/src/lib.rs",
                    "crates/application/src/other.rs",
                    "crates/order-execution/src/**/*.rs",
                    "crates/adapters/store-sqlite/src/**/*.rs"
                ]
                .map(String::from)
            )
            .expect("union"),
            [
                "application",
                "domain",
                "order-execution",
                "store-sqlite",
                "testkit-ports"
            ]
            .map(String::from)
            .into()
        );
        for path in [
            "crates/domain-other/src/lib.rs",
            "crates/domain/tests/test.rs",
            "crates/unknown/src/lib.rs",
            "other/crates/application/src/lib.rs",
            "crates/adapters/src/lib.rs",
            "crates/application/src-other/lib.rs",
        ] {
            assert_eq!(
                super::selected(&[String::from(path)]),
                Err(format!("변이 테스트 소유 선언이 없다: {path}"))
            );
        }
        super::selected(&[
            String::from("crates/domain/src/lib.rs"),
            String::from("unknown"),
        ])
        .expect_err("unknown owner after known owner");
    }
}
