use quote::quote;
use std::collections::BTreeSet;

fn wire_name(variant: &syn::Variant) -> Result<String, String> {
    for attr in variant
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
    {
        let items = attr
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .map_err(|error| error.to_string())?;
        for meta in items {
            if let syn::Meta::NameValue(value) = meta
                && value.path.is_ident("rename")
                && let syn::Expr::Lit(literal) = value.value
                && let syn::Lit::Str(text) = literal.lit
            {
                return Ok(text.value());
            }
        }
    }
    Ok(variant.ident.to_string())
}

pub(super) fn apply(file: &mut syn::File) -> Result<(), String> {
    let mut changed = BTreeSet::new();
    let mut implementations = Vec::new();
    for item in &mut file.items {
        let syn::Item::Enum(enumeration) = item else {
            continue;
        };
        if !enumeration
            .variants
            .iter()
            .all(|variant| variant.fields.is_empty())
        {
            continue;
        }
        let name = &enumeration.ident;
        let variants: Vec<syn::Ident> = enumeration
            .variants
            .iter()
            .map(|variant| variant.ident.clone())
            .collect();
        let values = enumeration
            .variants
            .iter()
            .map(wire_name)
            .collect::<Result<Vec<_>, _>>()?;
        changed.insert(name.to_string());
        enumeration.attrs = vec![
            syn::parse_quote!(#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, ::serde::Deserialize, ::serde::Serialize)]),
            syn::parse_quote!(#[serde(from = "String", into = "String")]),
        ];
        enumeration
            .variants
            .push(syn::parse_quote!(Unknown(::std::string::String)));
        let code = generated_impl(name, &variants, &values);
        implementations.extend(
            syn::parse2::<syn::File>(code)
                .map_err(|error| error.to_string())?
                .items,
        );
    }
    file.items.retain(|item| {
        if let syn::Item::Impl(implementation) = item
            && let syn::Type::Path(owner) = implementation.self_ty.as_ref()
        {
            return !owner
                .path
                .segments
                .last()
                .is_some_and(|segment| changed.contains(&segment.ident.to_string()));
        }
        true
    });
    file.items.extend(implementations);
    Ok(())
}

fn generated_impl(
    name: &syn::Ident,
    variants: &[syn::Ident],
    values: &[String],
) -> proc_macro2::TokenStream {
    quote! {
        impl From<String> for #name {
            fn from(value: String) -> Self {
                match value.as_str() { #(#values => Self::#variants,)* _ => Self::Unknown(value) }
            }
        }
        impl From<#name> for String {
            fn from(value: #name) -> Self {
                match value { #(#name::#variants => Self::from(#values),)* #name::Unknown(raw) => raw }
            }
        }
        impl ::std::fmt::Display for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self { #(Self::#variants => f.write_str(#values),)* Self::Unknown(raw) => f.write_str(raw) }
            }
        }
        impl ::std::str::FromStr for #name {
            type Err = ::std::convert::Infallible;
            fn from_str(value: &str) -> Result<Self, Self::Err> { Ok(Self::from(String::from(value))) }
        }
    }
}
