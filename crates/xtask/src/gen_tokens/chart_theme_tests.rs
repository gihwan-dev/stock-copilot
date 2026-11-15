use std::fs;

use super::{INPUT, json, render};

#[test]
fn chart_theme_scopes_contain_their_own_token_values() {
    let root = std::env::current_dir().expect("cwd").join("../..");
    let source = fs::read_to_string(root.join(INPUT)).expect("tokens");
    let data = json::parse(&source).expect("input");
    let (css, _) = render(&data).expect("render");
    for (theme, background) in [("light", "#f5f5f2"), ("dark", "#0e1012")] {
        let selector = format!("[data-chart-theme=\"{theme}\"] {{\n");
        let (_, after) = css.split_once(&selector).expect("scope");
        let (block, _) = after.split_once("}\n").expect("block");
        assert!(block.contains(&format!("--color-bg: {background};")));
        assert!(block.contains("--color-chart-1:"));
        assert_eq!(css.matches(&selector).count(), 1);
    }
}
