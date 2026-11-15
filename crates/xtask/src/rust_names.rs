use proc_macro2::Ident;

pub(crate) fn normalize(name: &str) -> String {
    String::from(name.strip_prefix("r#").unwrap_or(name))
}

pub(crate) fn ident(name: &Ident) -> String {
    normalize(&name.to_string())
}
