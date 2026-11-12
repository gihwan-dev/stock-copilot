fn without_element(items: &[toml::Value], position: usize) -> Vec<toml::Value> {
    items
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != position)
        .map(|(_, item)| item.clone())
        .collect()
}

pub(crate) fn element_variants(value: &toml::Value, label: &str) -> Vec<(String, toml::Value)> {
    match value {
        toml::Value::Table(table) => table
            .iter()
            .flat_map(|(key, child)| {
                let mut removed = table.clone();
                removed.remove(key);
                let own = (
                    format!("{label}.{key} removed"),
                    toml::Value::Table(removed),
                );
                let nested = element_variants(child, &format!("{label}.{key}"))
                    .into_iter()
                    .map(|(name, variant)| {
                        let mut replaced = table.clone();
                        replaced.insert(key.clone(), variant);
                        (name, toml::Value::Table(replaced))
                    })
                    .collect::<Vec<_>>();
                std::iter::once(own).chain(nested)
            })
            .collect(),
        toml::Value::Array(items) => (0..items.len())
            .flat_map(|position| {
                let removed = (
                    format!("{label}[{position}] removed"),
                    toml::Value::Array(without_element(items, position)),
                );
                let nested = items
                    .get(position)
                    .map(|item| element_variants(item, &format!("{label}[{position}]")))
                    .into_iter()
                    .flatten()
                    .map(|(name, variant)| {
                        let mut replaced = items.clone();
                        if let Some(slot) = replaced.get_mut(position) {
                            *slot = variant;
                        }
                        (name, toml::Value::Array(replaced))
                    })
                    .collect::<Vec<_>>();
                std::iter::once(removed).chain(nested)
            })
            .collect(),
        toml::Value::String(_)
        | toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => Vec::new(),
    }
}
