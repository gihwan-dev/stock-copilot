use std::collections::BTreeSet;

pub(super) fn selected(globs: &[String]) -> Result<BTreeSet<String>, String> {
    let declarations = [
        ("crates/domain", "domain", None),
        ("crates/application", "application", Some("testkit-ports")),
        ("crates/order-execution", "order-execution", None),
        ("crates/adapters/store-sqlite", "store-sqlite", None),
    ];
    let mut packages = BTreeSet::new();
    for glob in globs {
        let (_, package, companion) = declarations
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

#[cfg(test)]
mod tests {
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
