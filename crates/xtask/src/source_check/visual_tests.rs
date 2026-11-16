use std::collections::BTreeSet;

use super::allowlist::Allowlists;
use super::finding::{Finding, Rule};
use super::{Entry, EntryKind, entry_text_findings};

#[test]
fn ds15_darwin_baseline_is_rejected_without_reading_binary_data() {
    let root = super::repository_root().expect("repository root");
    let allowlists = Allowlists::load(&root).expect("source allowlists");
    for path in [
        "snapshots/button-darwin.png",
        "elsewhere/a-darwin.PNG",
        "a-darwin.jpeg",
        "a-darwin.jpg",
        "a-darwin.webp",
        "Button-light-chromium-darwin-arm64.png",
        "Money-dark-webkit-darwin-x64.PNG",
    ] {
        let entry = Entry {
            relative: path.into(),
            name: path.rsplit('/').next().expect("file name").into(),
            kind: EntryKind::File,
        };
        let found = entry_text_findings(&entry, &[], &allowlists, || {
            panic!("binary must not be read")
        })
        .expect("entry findings");
        assert_eq!(
            found.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([Finding::whole_file(path, Rule::DarwinBaseline)])
        );
    }
    assert_eq!(Rule::DarwinBaseline.to_string(), "file:darwin-baseline");
}

#[test]
fn ds15_linux_baseline_and_non_images_are_allowed() {
    let root = super::repository_root().expect("repository root");
    let allowlists = Allowlists::load(&root).expect("source allowlists");
    for path in ["a-chromium-linux-arm64.png", "a-darwin.json", "darwin.png"] {
        let entry = Entry {
            relative: path.into(),
            name: path.into(),
            kind: EntryKind::File,
        };
        assert_eq!(
            entry_text_findings(&entry, &[], &allowlists, || panic!("not source"))
                .expect("entry findings"),
            Vec::new()
        );
    }
}
