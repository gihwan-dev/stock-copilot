use std::collections::BTreeSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::process::{Command, Stdio};

use crate::capability_check::json::{self, Json};

const INPUT: &str = "apps/desktop/design-tokens/tokens.json";
const CSS: &str = "apps/desktop/src/shared/ds/tokens.css";
const TYPES: &str = "apps/desktop/src/shared/ds/tokens.ts";
const GROUPS: [(&str, &str); 11] = [
    ("color", "ColorToken"),
    ("spacing", "SpacingToken"),
    ("radius", "RadiusToken"),
    ("shadow", "ShadowToken"),
    ("size", "SizeToken"),
    ("duration", "DurationToken"),
    ("easing", "EasingToken"),
    ("zIndex", "ZIndexToken"),
    ("opacity", "OpacityToken"),
    ("layout", "LayoutToken"),
    ("text", "TextToken"),
];

type Generated = (String, String);

fn field<'a>(data: &'a Json, name: &str) -> Result<&'a Json, String> {
    data.get(name).ok_or_else(|| format!("missing {name}"))
}

fn text<'a>(data: &'a Json, name: &str) -> Result<&'a str, String> {
    field(data, name)?
        .as_str()
        .ok_or_else(|| format!("invalid text {name}"))
}

fn array(data: &Json) -> Result<&[Json], String> {
    data.as_array()
        .ok_or_else(|| String::from("expected array"))
}

fn rem(value: &str) -> Result<String, String> {
    let raw = value
        .strip_suffix("px")
        .ok_or_else(|| format!("not px: {value}"))?;
    let (sign, unsigned) = raw.strip_prefix('-').map_or(("", raw), |rest| ("-", rest));
    let places = unsigned
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let digits = unsigned
        .replace('.', "")
        .parse::<u64>()
        .map_err(|error| error.to_string())?;
    let scaled = digits
        .checked_mul(625)
        .ok_or_else(|| String::from("rem overflow"))?;
    let width = places
        .checked_add(4)
        .ok_or_else(|| String::from("rem precision overflow"))?;
    let denominator = 10_u64
        .checked_pow(u32::try_from(width).map_err(|error| error.to_string())?)
        .ok_or_else(|| String::from("rem precision overflow"))?;
    let whole = scaled
        .checked_div(denominator)
        .ok_or_else(|| String::from("rem divide"))?;
    let remainder = scaled
        .checked_rem(denominator)
        .ok_or_else(|| String::from("rem remainder"))?;
    if remainder == 0 {
        return Ok(format!("{sign}{whole}rem"));
    }
    let fraction = format!("{remainder:0width$}");
    Ok(format!(
        "{sign}{whole}.{}rem",
        fraction.trim_end_matches('0')
    ))
}

fn mapped(group: &str, name: &str, value: &str) -> Result<(String, String), String> {
    let direct = |prefix: &str, suffix: &str| (format!("--{prefix}-{suffix}"), String::from(value));
    let length = |prefix: &str, suffix: &str| {
        rem(value).map(|converted| (format!("--{prefix}-{suffix}"), converted))
    };
    match group {
        "color" => Ok((
            format!("--color-{name}"),
            value
                .strip_prefix('{')
                .and_then(|rest| rest.strip_suffix('}'))
                .map_or_else(
                    || String::from(value),
                    |reference| format!("var(--color-{reference})"),
                ),
        )),
        "radius" => length(
            "radius",
            name.strip_prefix("radius-")
                .ok_or_else(|| String::from("radius prefix"))?,
        ),
        "shadow" => Ok((format!("--{name}"), format!("var(--sc-{name})"))),
        "size" if name.starts_with("border-") => {
            Ok(direct("border-width", name.trim_start_matches("border-")))
        }
        "size" if name == "focus-offset" => {
            Ok((String::from("--sc-focus-offset"), String::from(value)))
        }
        "size" if name == "bubble-max-width" => Ok(direct("spacing", name)),
        "duration" => Ok(direct(
            "transition-duration",
            name.trim_start_matches("duration-"),
        )),
        "easing" => Ok(direct("ease", name.trim_start_matches("ease-"))),
        "zIndex" => Ok(direct("z-index", name.trim_start_matches("z-"))),
        "opacity" => Ok(direct("opacity", name.trim_start_matches("opacity-"))),
        "layout" if name == "grid-columns" => {
            Ok((String::from("--sc-grid-columns"), String::from(value)))
        }
        "layout" if name.starts_with("bp-window-") => {
            length("breakpoint", name.trim_start_matches("bp-"))
        }
        "layout" if name.starts_with("bp-content-") => {
            length("container", name.trim_start_matches("bp-"))
        }
        "spacing" | "size" | "layout" => length("spacing", name),
        "text" if name.starts_with("text-") => {
            length("text-size", name.trim_start_matches("text-"))
        }
        "text" if name.starts_with("leading-") => {
            length("leading", name.trim_start_matches("leading-"))
        }
        "text" if name.starts_with("weight-") => {
            Ok(direct("font-weight", name.trim_start_matches("weight-")))
        }
        "text" if name.starts_with("tracking-") => {
            length("tracking", name.trim_start_matches("tracking-"))
        }
        _ => Err(format!("unsupported token: {group}/{name}")),
    }
}

fn declaration(output: &mut String, name: &str, value: &str) {
    let _written = writeln!(output, "  {name}: {value};");
}

fn tokens_for(data: &Json, theme: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut names = BTreeSet::new();
    for (group, _) in GROUPS {
        for token in array(field(field(data, group)?, "tokens")?)? {
            let name = text(token, "name")?;
            let value = text(field(token, "value")?, theme)?;
            let (variable, converted) = mapped(group, name, value)?;
            if !names.insert(variable.clone()) {
                return Err(format!("duplicate variable {variable}"));
            }
            declaration(&mut output, &variable, &converted);
            if group == "shadow" {
                declaration(&mut output, &format!("--sc-{name}"), value);
            }
        }
    }
    let typography = field(data, "type")?;
    let families = field(typography, "families")?;
    for family in ["sans", "mono"] {
        declaration(
            &mut output,
            &format!("--font-{family}"),
            text(families, family)?,
        );
    }
    for group in array(field(typography, "groups")?)? {
        for style in array(field(group, "styles")?)? {
            let name = text(style, "name")?;
            let variable = format!("--text-{name}");
            if !names.insert(variable.clone()) {
                return Err(format!("duplicate style {name}"));
            }
            declaration(&mut output, &variable, &rem(text(style, "fontSize")?)?);
            declaration(
                &mut output,
                &format!("{variable}--line-height"),
                &rem(text(style, "lineHeight")?)?,
            );
            let Json::Number(weight) = field(style, "fontWeight")? else {
                return Err(String::from("fontWeight must be integer"));
            };
            declaration(&mut output, &format!("{variable}--font-weight"), weight);
            if let Some(Json::Text(value)) = style.get("letterSpacing") {
                declaration(
                    &mut output,
                    &format!("{variable}--letter-spacing"),
                    &rem(value)?,
                );
            }
        }
    }
    Ok(output)
}

fn union(output: &mut String, name: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() {
        return Err(format!("empty union {name}"));
    }
    let variants: Vec<String> = values.iter().map(|value| format!("\"{value}\"")).collect();
    let _written = write!(output, "export type {name} = {};\n\n", variants.join(" | "));
    Ok(())
}

fn font_faces(data: &Json, css: &mut String) -> Result<(), String> {
    for font in array(field(field(data, "type")?, "fonts")?)? {
        let path = text(font, "file")?;
        if !path.starts_with("fonts/")
            || path.contains("..")
            || Path::new(path)
                .extension()
                .is_none_or(|extension| extension != "woff2")
        {
            return Err(String::from("invalid font path"));
        }
        let _written = write!(
            css,
            "@font-face {{\n  font-family: \"{}\";\n  src: url(\"../../../../../design/{path}\") format(\"woff2\");\n  font-weight: {};\n  font-style: {};\n  font-display: swap;\n}}\n\n",
            text(font, "family")?,
            text(font, "weight")?,
            text(font, "style")?
        );
    }
    Ok(())
}

fn render(data: &Json) -> Result<Generated, String> {
    let light = tokens_for(data, "light")?;
    let dark = tokens_for(data, "dark")?;
    let mut css = String::from("@theme {\n  --*: initial;\n");
    css.push_str(&light);
    css.push_str("}\n\n:root {\n");
    css.push_str(&light);
    css.push_str("}\n\n@media (prefers-color-scheme: dark) {\n:root:not([data-theme]) {\n");
    css.push_str(&dark);
    css.push_str("}\n}\n\n:root[data-theme=\"dark\"] {\n");
    css.push_str(&dark);
    css.push_str("}\n\n");
    for token in array(field(field(data, "size")?, "tokens")?)? {
        let name = text(token, "name")?;
        if let Some(suffix) = name.strip_prefix("border-") {
            let _written = write!(
                css,
                "@utility border-{suffix} {{\n  border-style: solid;\n  border-width: var(--border-width-{suffix});\n}}\n\n"
            );
        }
    }
    css.push_str("@utility outline-offset-focus {\n  outline-offset: var(--sc-focus-offset);\n}\n\n@utility grid-cols-layout {\n  grid-template-columns: repeat(var(--sc-grid-columns), minmax(0, 1fr));\n}\n\n");
    font_faces(data, &mut css)?;
    let mut types = String::new();
    let mut groups = Vec::new();
    for (group, type_name) in GROUPS {
        let names: Result<Vec<String>, String> = array(field(field(data, group)?, "tokens")?)?
            .iter()
            .map(|token| text(token, "name").map(String::from))
            .collect();
        union(&mut types, type_name, &names?)?;
        groups.push(type_name);
    }
    let mut styles = Vec::new();
    for group in array(field(field(data, "type")?, "groups")?)? {
        for style in array(field(group, "styles")?)? {
            styles.push(String::from(text(style, "name")?));
        }
    }
    union(&mut types, "TypeStyleToken", &styles)?;
    union(
        &mut types,
        "FontToken",
        &[String::from("sans"), String::from("mono")],
    )?;
    groups.extend(["TypeStyleToken", "FontToken"]);
    let _written = writeln!(types, "export type TokenName = {};", groups.join(" | "));
    Ok((css, types))
}

fn formatted(path: &str, content: &str) -> Result<String, String> {
    let mut child = Command::new("node_modules/.bin/prettier")
        .args(["--stdin-filepath", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    child
        .stdin
        .take()
        .ok_or_else(|| String::from("formatter stdin"))?
        .write_all(content.as_bytes())
        .map_err(|error| error.to_string())?;
    let result = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into_owned());
    }
    String::from_utf8(result.stdout).map_err(|error| error.to_string())
}

fn run(arguments: &[String]) -> Result<(), String> {
    let check = match arguments {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err(String::from("usage: xtask gen-tokens [--check]")),
    };
    let input = fs::read_to_string(INPUT).map_err(|error| error.to_string())?;
    let parsed = json::parse(&input).ok_or_else(|| String::from("invalid tokens JSON"))?;
    let (css, types) = render(&parsed)?;
    for (path, raw) in [(CSS, css), (TYPES, types)] {
        let generated = formatted(path, &raw)?;
        if check {
            if fs::read_to_string(path).map_err(|error| error.to_string())? != generated {
                return Err(format!(
                    "{path}: 재생성 차이, xtask gen-tokens를 실행하세요"
                ));
            }
        } else {
            let parent = Path::new(path)
                .parent()
                .ok_or_else(|| String::from("output parent"))?;
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            fs::write(path, generated).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn command(arguments: &[String]) -> ExitCode {
    match run(arguments) {
        Ok(()) => {
            let _output = writeln!(io::stdout().lock(), "gen-tokens: 위반 없음");
            ExitCode::SUCCESS
        }
        Err(error) => {
            let _output = writeln!(io::stderr().lock(), "gen-tokens: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
