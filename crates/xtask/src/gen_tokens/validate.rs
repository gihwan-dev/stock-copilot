use std::collections::BTreeSet;

use super::{GROUPS, Json, array, field, text};

fn name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!("invalid token name: {value}"));
    }
    Ok(())
}

fn unsigned(value: &str) -> Option<u64> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}

fn decimal(value: &str) -> bool {
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or("");
    let fraction = parts.next();
    unsigned(integer).is_some()
        && fraction
            .is_none_or(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        && parts.next().is_none()
}

fn pixel(value: &str) -> bool {
    value.strip_suffix("px").is_some_and(decimal)
}

fn rgba(value: &str) -> bool {
    let Some(inner) = value
        .strip_prefix("rgba(")
        .and_then(|part| part.strip_suffix(')'))
    else {
        return false;
    };
    let mut parts = inner.split(", ");
    for _ in 0_u8..3 {
        if parts.next().is_none_or(|part| part.parse::<u8>().is_err()) {
            return false;
        }
    }
    let alpha = parts.next().is_some_and(ratio);
    alpha && parts.next().is_none()
}

fn ratio(value: &str) -> bool {
    value == "0"
        || value == "1"
        || value
            .strip_prefix("0.")
            .is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn color(value: &str) -> bool {
    value
        .strip_prefix('#')
        .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        || rgba(value)
        || value
            .strip_prefix('{')
            .and_then(|part| part.strip_suffix('}'))
            .is_some_and(|reference| name(reference).is_ok())
}

fn shadow(value: &str) -> bool {
    value
        .split(')')
        .filter(|part| !part.is_empty())
        .all(|part| {
            let segment = part.strip_prefix(", ").unwrap_or(part);
            let Some((length_text, channels)) = segment.split_once(" rgba(") else {
                return false;
            };
            let lengths: Vec<&str> = length_text.split(' ').collect();
            (3..=4).contains(&lengths.len())
                && lengths.iter().all(|length| *length == "0" || pixel(length))
                && rgba(&format!("rgba({channels})"))
        })
        && value.ends_with(')')
}

fn easing(value: &str) -> bool {
    let Some(inner) = value
        .strip_prefix("cubic-bezier(")
        .and_then(|part| part.strip_suffix(')'))
    else {
        return false;
    };
    let parts: Vec<&str> = inner.split(", ").collect();
    parts.len() == 4 && parts.iter().all(|part| ratio(part))
}

fn valid_value(group: &str, token: &str, value: &str) -> bool {
    match group {
        "color" => color(value),
        "shadow" => shadow(value),
        "duration" => value
            .strip_suffix("ms")
            .is_some_and(|part| unsigned(part).is_some()),
        "easing" => easing(value),
        "zIndex" => unsigned(value).is_some(),
        "opacity" => ratio(value),
        "layout" if token == "grid-columns" => unsigned(value).is_some_and(|count| count > 0),
        "size" if token == "bubble-max-width" => value
            .strip_suffix('%')
            .is_some_and(|part| unsigned(part).is_some_and(|percent| (1..=100).contains(&percent))),
        "text" if token.starts_with("weight-") => [
            "100", "200", "300", "400", "500", "600", "700", "800", "900",
        ]
        .contains(&value),
        "text" if token.starts_with("tracking-") => pixel(value.strip_prefix('-').unwrap_or(value)),
        "spacing" | "radius" | "size" | "layout" | "text" => pixel(value),
        _ => false,
    }
}

fn token_values(token: &Json, group: &str, token_name: &str) -> Result<(), String> {
    let Json::Object(values) = field(token, "value")? else {
        return Err(String::from("theme values must be object"));
    };
    if values.len() != 2 {
        return Err(String::from("expected light and dark only"));
    }
    for theme in ["light", "dark"] {
        let value = text(field(token, "value")?, theme)?;
        if !valid_value(group, token_name, value) {
            return Err(format!(
                "invalid value {group}/{token_name}/{theme}: {value}"
            ));
        }
    }
    Ok(())
}

fn group_values(data: &Json) -> Result<(), String> {
    for (group, _) in GROUPS {
        let mut names = BTreeSet::new();
        for token in array(field(field(data, group)?, "tokens")?)? {
            let token_name = text(token, "name")?;
            name(token_name)?;
            if !names.insert(token_name) {
                return Err(format!("duplicate token {token_name}"));
            }
            token_values(token, group, token_name)?;
        }
    }
    Ok(())
}

fn reference(tokens: &[Json], token: &Json, theme: &str) -> Result<(), String> {
    let mut visited = BTreeSet::new();
    let mut current = text(token, "name")?;
    loop {
        if !visited.insert(current) {
            return Err(format!("cyclic color {current}"));
        }
        let next = tokens
            .iter()
            .find(|item| text(item, "name") == Ok(current))
            .ok_or_else(|| format!("unresolved color {current}"))?;
        let value = text(field(next, "value")?, theme)?;
        let Some(reference) = value
            .strip_prefix('{')
            .and_then(|part| part.strip_suffix('}'))
        else {
            break;
        };
        current = reference;
    }
    Ok(())
}

fn references(data: &Json) -> Result<(), String> {
    let tokens = array(field(field(data, "color")?, "tokens")?)?;
    for theme in ["light", "dark"] {
        for token in tokens {
            reference(tokens, token, theme)?;
        }
    }
    Ok(())
}

fn fonts(typography: &Json) -> Result<(), String> {
    for font in array(field(typography, "fonts")?)? {
        let family = text(font, "family")?;
        if family.is_empty()
            || !family
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == ' ' || ch == '-')
        {
            return Err(String::from("invalid font family"));
        }
        let path = text(font, "file")?;
        if !path.starts_with("fonts/")
            || path.contains("..")
            || !path
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/-_.".contains(&byte))
        {
            return Err(String::from("invalid font path"));
        }
        if !["400", "500", "600", "700"].contains(&text(font, "weight")?)
            || text(font, "style")? != "normal"
        {
            return Err(String::from("invalid font face"));
        }
    }
    Ok(())
}

fn type_style(style: &Json) -> Result<(), String> {
    name(text(style, "name")?)?;
    if !pixel(text(style, "fontSize")?) || !pixel(text(style, "lineHeight")?) {
        return Err(String::from("invalid type length"));
    }
    let Json::Number(weight) = field(style, "fontWeight")? else {
        return Err(String::from("invalid weight"));
    };
    if !["400", "500", "600", "700"].contains(&weight.as_str()) {
        return Err(String::from("invalid type weight"));
    }
    if let Some(value) = style.get("letterSpacing") {
        let Json::Text(spacing) = value else {
            return Err(String::from("invalid tracking"));
        };
        if !pixel(spacing.strip_prefix('-').unwrap_or(spacing)) {
            return Err(String::from("invalid tracking"));
        }
    }
    Ok(())
}

fn typography(data: &Json) -> Result<(), String> {
    let typography = field(data, "type")?;
    let Json::Object(families) = field(typography, "families")? else {
        return Err(String::from("invalid families"));
    };
    for (family, value) in families {
        name(family)?;
        let Json::Text(stack) = value else {
            return Err(String::from("invalid font stack"));
        };
        if !stack
            .chars()
            .all(|ch| ch.is_alphanumeric() || " ,\"-_".contains(ch))
        {
            return Err(String::from("invalid font stack"));
        }
    }
    for group in array(field(typography, "groups")?)? {
        if !families.contains_key(text(group, "family")?) {
            return Err(String::from("unresolved font family"));
        }
        for style in array(field(group, "styles")?)? {
            type_style(style)?;
        }
    }
    fonts(typography)
}

pub(super) fn validate(data: &Json) -> Result<(), String> {
    let Json::Object(root) = data else {
        return Err(String::from("tokens must be object"));
    };
    if let Some(unknown) = root.keys().find(|key| {
        !["name", "version", "type"].contains(&key.as_str())
            && !GROUPS.iter().any(|(group, _)| key == group)
    }) {
        return Err(format!("unknown group {unknown}"));
    }
    let theme_ids = array(field(field(data, "color")?, "themes")?)?
        .iter()
        .map(|theme| text(theme, "id"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if theme_ids != BTreeSet::from(["light", "dark"]) {
        return Err(String::from("invalid theme IDs"));
    }
    group_values(data)?;
    references(data)?;
    typography(data)
}
