use std::fs;

use super::{INPUT, Json, json, rem, render};

fn input() -> Json {
    let root = std::env::current_dir().expect("cwd").join("../..");
    let text = fs::read_to_string(root.join(INPUT)).expect("tokens input");
    json::parse(&text).expect("JSON")
}

fn remove_token(data: &mut Json, name: &str, index: usize) {
    let Json::Object(groups) = data else {
        return;
    };
    let Some(Json::Object(group)) = groups.get_mut(name) else {
        return;
    };
    let Some(Json::Array(items)) = group.get_mut("tokens") else {
        return;
    };
    *items = items
        .iter()
        .enumerate()
        .filter(|(position, _)| *position != index)
        .map(|(_, item)| item.clone())
        .collect();
}

#[test]
fn generation_preserves_every_mapping_and_exact_units() {
    let (css, types) = render(&input()).expect("render");
    for expected in [
        "--*: initial;",
        "--color-bg: #f5f5f2;",
        "--spacing-space-4: 1rem;",
        "--text-size-xs: 0.6875rem;",
        "--text-body: 0.875rem;",
        "--text-body--line-height: 1.375rem;",
        "--text-body--font-weight: 400;",
        "--spacing-content-max: 75rem;",
        "--text-num-display--letter-spacing: -0.03125rem;",
        "--radius-md: 0.5rem;",
        "--shadow-popover: var(--sc-shadow-popover);",
        "--transition-duration-fast: 120ms;",
        "--ease-out: cubic-bezier(",
        "--z-index-modal: 300;",
        "--opacity-disabled: 0.45;",
        "--breakpoint-window-md: 67.5rem;",
        "--container-content-medium: 56.25rem;",
        "--sc-grid-columns: 12;",
        "repeat(var(--sc-grid-columns), minmax(0, 1fr))",
        "--spacing-bubble-max-width: 80%;",
        "--border-width-mark: 1.5px;",
        "--spacing-icon-sm: 1rem;",
        "--sc-focus-offset: 2px;",
        "--font-weight-semibold: 600;",
        "--leading-xs: 1rem;",
        "--tracking-display: -0.03125rem;",
        "--font-sans:",
        "../../../../../design/fonts/IBMPlexSansKR-Regular.woff2",
        ":root:not([data-theme])",
        ":root[data-theme=\"dark\"]",
        "--color-bg: #0e1012;",
    ] {
        assert!(css.contains(expected), "missing {expected}");
    }
    assert!(!css.contains("--text-text-"));
    assert!(!css.contains("--shadow-shadow-"));
    assert!(!css.contains("--text-sm:"));
    assert!(!types.contains("#f5f5f2"));
    for name in [
        "ColorToken",
        "SpacingToken",
        "SizeToken",
        "LayoutToken",
        "TextToken",
        "TypeStyleToken",
        "TokenName",
    ] {
        assert!(types.contains(name), "missing {name}");
    }
}

#[test]
fn every_input_element_changes_generation_and_missing_sections_fail() {
    let data = input();
    let original = render(&data).expect("render");
    let Json::Object(sections) = &data else {
        panic!("object");
    };
    {
        for (name, section) in sections {
            let Some(Json::Array(tokens)) = section.get("tokens") else {
                continue;
            };
            for index in 0..tokens.len() {
                let mut changed = data.clone();
                remove_token(&mut changed, name, index);
                assert_ne!(render(&changed), Ok(original.clone()), "{name} {index}");
            }
        }
        for name in [
            "color", "spacing", "radius", "shadow", "size", "duration", "easing", "zIndex",
            "opacity", "layout", "text", "type",
        ] {
            let mut changed = sections.clone();
            changed.remove(name);
            assert!(render(&Json::Object(changed)).is_err(), "{name}");
        }
    }
}

#[test]
fn integer_rem_conversion_rejects_invalid_values() {
    for (source, expected) in [
        ("32px", "2rem"),
        ("1.5px", "0.09375rem"),
        ("-0.5px", "-0.03125rem"),
        ("0px", "0rem"),
    ] {
        assert_eq!(rem(source).expect("rem"), expected);
    }
    for source in ["x", "NaNpx", "999999999999999999999px"] {
        assert!(rem(source).is_err(), "{source}");
    }
}
