use serde_json::Value;

fn resolve<'a>(spec: &'a Value, schema: &'a Value) -> Result<&'a Value, String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        spec.pointer(reference.strip_prefix('#').ok_or("external reference")?)
            .ok_or_else(|| format!("missing reference {reference}"))
    } else {
        Ok(schema)
    }
}

pub(crate) fn validate_recording(spec: &Value, recording: &Value) -> Result<(), String> {
    if recording["origin"] != "synthetic" || recording["method"] != "get" {
        return Err(String::from("synthetic GET required"));
    }
    let path = recording["path"].as_str().ok_or("path")?;
    let status = recording["status"].as_str().ok_or("status")?;
    let response = spec["paths"]
        .get(path)
        .and_then(|node| node.get("get"))
        .and_then(|node| node.get("responses"))
        .and_then(|node| node.get(status))
        .ok_or("unknown path/status")?;
    let resolved_response = resolve(spec, response)?;
    let schema = resolved_response
        .pointer("/content/application~1json/schema")
        .ok_or("response schema")?;
    let declared = recording["schema"]
        .as_str()
        .and_then(|node| node.strip_prefix('#'))
        .and_then(|node| spec.pointer(node))
        .ok_or("declared schema")?;
    if schema != declared {
        return Err(String::from("wrong schema"));
    }
    let body = recording.get("body").ok_or("body")?;
    validate(spec, schema, body, 0)?;
    if status == "200" {
        serde_json::from_value::<broker_toss_schema::types::ApiResponse>(body.clone())
            .map_err(|error| error.to_string())?;
    } else {
        serde_json::from_value::<broker_toss_schema::types::ErrorResponse>(body.clone())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn validate(
    spec: &Value,
    raw_schema: &Value,
    value: &Value,
    depth: usize,
) -> Result<(), String> {
    if depth > 64 {
        return Err(String::from("schema recursion limit"));
    }
    let schema = resolve(spec, raw_schema)?;
    check_keywords(schema)?;
    if schema.get("$ref").is_some() {
        return validate(spec, schema, value, depth.saturating_add(1));
    }
    if schema.get("nullable") == Some(&Value::Bool(true)) && value.is_null() {
        return Ok(());
    }
    validate_extras(spec, schema, value, depth)?;
    validate_one_of(spec, schema, value, depth)?;
    if let Some(branches) = schema.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            validate(spec, branch, value, depth.saturating_add(1))?;
        }
    }
    if let Some(kind) = schema.get("type") {
        let accepted = match kind {
            Value::String(name) => type_matches(name, value),
            Value::Array(names) => names
                .iter()
                .filter_map(Value::as_str)
                .any(|name| type_matches(name, value)),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => false,
        };
        if !accepted {
            return Err(String::from("type mismatch"));
        }
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array)
        && !values.contains(value)
    {
        return Err(String::from("enum mismatch"));
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required {
            if value.get(name.as_str().ok_or("required name")?).is_none() {
                return Err(format!("missing {name}"));
            }
        }
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, property) in properties {
            if let Some(field) = value.get(name) {
                validate(spec, property, field, depth.saturating_add(1))?;
            }
        }
    }
    if let Some(items) = schema.get("items") {
        for item in value.as_array().ok_or("array")? {
            validate(spec, items, item, depth.saturating_add(1))?;
        }
    }
    validate_length(schema, value)
}

fn validate_length(schema: &Value, value: &Value) -> Result<(), String> {
    if let (Some(max), Some(text)) = (
        schema.get("maxLength").and_then(Value::as_u64),
        value.as_str(),
    ) && u64::try_from(text.chars().count()).map_err(|error| error.to_string())? > max
    {
        return Err(String::from("maxLength"));
    }
    Ok(())
}

fn validate_extras(
    spec: &Value,
    schema: &Value,
    value: &Value,
    depth: usize,
) -> Result<(), String> {
    let Some(fields) = value.as_object() else {
        return Ok(());
    };
    for (name, field) in fields {
        if schema
            .get("properties")
            .and_then(|properties| properties.get(name))
            .is_some()
        {
            continue;
        }
        match schema.get("additionalProperties") {
            Some(Value::Bool(false)) => return Err(String::from("extra field")),
            Some(additional @ Value::Object(_)) => {
                validate(spec, additional, field, depth.saturating_add(1))?;
            }
            Some(Value::Bool(true)) | None => (),
            Some(other) => return Err(format!("unsupported additionalProperties {other}")),
        }
    }
    Ok(())
}

fn validate_one_of(
    spec: &Value,
    schema: &Value,
    value: &Value,
    depth: usize,
) -> Result<(), String> {
    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        let matches = branches
            .iter()
            .filter(|branch| validate(spec, branch, value, depth.saturating_add(1)).is_ok())
            .count();
        if matches != 1 {
            return Err(String::from("oneOf mismatch"));
        }
    }
    Ok(())
}

fn check_keywords(schema: &Value) -> Result<(), String> {
    let object = schema.as_object().ok_or("schema object")?;
    for key in object.keys() {
        if ![
            "$ref",
            "type",
            "properties",
            "required",
            "items",
            "enum",
            "allOf",
            "oneOf",
            "description",
            "example",
            "format",
            "maxLength",
            "additionalProperties",
            "nullable",
            "title",
            "default",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unsupported schema keyword {key}"));
        }
    }
    Ok(())
}

fn type_matches(name: &str, value: &Value) -> bool {
    match name {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}
