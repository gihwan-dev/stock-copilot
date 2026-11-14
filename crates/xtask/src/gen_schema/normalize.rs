use quote::ToTokens;
use std::collections::BTreeMap;
use syn::visit_mut::VisitMut;

fn pattern_expression(statement: &syn::Stmt) -> Option<&syn::Expr> {
    let syn::Stmt::Item(syn::Item::Static(item)) = statement else {
        return None;
    };
    if item.ident != "PATTERN" {
        return None;
    }
    let syn::Expr::Call(call) = item.expr.as_ref() else {
        return None;
    };
    let syn::Expr::Closure(closure) = call.args.first()? else {
        return None;
    };
    let body = if let syn::Expr::Block(block) = closure.body.as_ref() {
        let syn::Stmt::Expr(expression, _) = block.block.stmts.last()? else {
            return None;
        };
        expression
    } else {
        closure.body.as_ref()
    };
    let syn::Expr::MethodCall(method) = body else {
        return None;
    };
    (method.method == "unwrap").then_some(method.receiver.as_ref())
}

fn field_name(owner: &str, name: &str) -> Option<String> {
    let suffix = match owner {
        "AfterMarketSession" | "PreMarketSession" | "RegularMarketSession" => "_time",
        "IntegratedHour" => "_market",
        "InvestorTradingVolume" | "ProgramTradingVolume" => "_volume",
        _ => "",
    };
    if !suffix.is_empty() {
        return name.strip_suffix(suffix).map(String::from);
    }
    let prefix = match owner {
        "Account" => "account_",
        "Commission" => "commission_",
        "Order" => "order_",
        "Sector" => "sector_",
        "Transaction" => "transaction_",
        _ => return None,
    };
    name.strip_prefix(prefix)
        .map(|stripped| String::from(if stripped == "type" { "kind" } else { stripped }))
}

fn default_names(item: &syn::Item) -> Option<(String, String)> {
    let syn::Item::Impl(implementation) = item else {
        return None;
    };
    let (_, trait_path, _) = implementation.trait_.as_ref()?;
    if trait_path.segments.last()?.ident != "Default" {
        return None;
    }
    let syn::Type::Path(owner) = implementation.self_ty.as_ref() else {
        return None;
    };
    let syn::ImplItem::Fn(function) = implementation.items.first()? else {
        return None;
    };
    if function.sig.ident != "default" {
        return None;
    }
    let syn::Stmt::Expr(syn::Expr::Path(value), None) = function.block.stmts.first()? else {
        return None;
    };
    if value.path.segments.len() != 2
        || value.path.segments.first()?.ident != owner.path.segments.last()?.ident
    {
        return None;
    }
    Some((
        owner.path.segments.last()?.ident.to_string(),
        value.path.segments.last()?.ident.to_string(),
    ))
}

pub(super) fn normalize(stream: proc_macro2::TokenStream) -> Result<String, String> {
    let mut file = syn::parse2::<syn::File>(stream).map_err(|error| error.to_string())?;
    let defaults = file.items.iter().filter_map(default_names).collect();
    file.items.retain(|item| default_names(item).is_none());
    let mut visitor = Normalize { defaults };
    visitor.visit_file_mut(&mut file);
    Ok(file
        .into_token_stream()
        .to_string()
        .replace("self :: error ::", "error ::")
        .replace("value . to_string ()", "value . to_owned ()"))
}

struct Normalize {
    defaults: BTreeMap<String, String>,
}
impl VisitMut for Normalize {
    fn visit_ident_mut(&mut self, i: &mut syn::Ident) {
        if i == "e" {
            *i = syn::Ident::new("error", i.span());
        }
        if i == "PATTERN" {
            *i = syn::Ident::new("pattern", i.span());
        }
    }
    fn visit_lit_int_mut(&mut self, i: &mut syn::LitInt) {
        let suffix = i.suffix();
        if !suffix.is_empty() {
            *i = syn::LitInt::new(&format!("{}_{}", i.base10_digits(), suffix), i.span());
        }
    }
    fn visit_stmt_mut(&mut self, i: &mut syn::Stmt) {
        if let Some(receiver) = pattern_expression(i) {
            *i = syn::parse_quote!(let PATTERN = #receiver.map_err(|error| error.to_string())?;);
        }
        syn::visit_mut::visit_stmt_mut(self, i);
    }
    fn visit_item_struct_mut(&mut self, i: &mut syn::ItemStruct) {
        i.attrs.retain(|attr| !attr.path().is_ident("doc"));
        for field in &mut i.fields {
            field.attrs.retain(|attr| !attr.path().is_ident("doc"));
        }
        for field in &mut i.fields {
            if let Some(ident) = field.ident.as_mut()
                && let Some(name) = field_name(&i.ident.to_string(), &ident.to_string())
            {
                *ident = syn::Ident::new(&name, ident.span());
            }
        }
        syn::visit_mut::visit_item_struct_mut(self, i);
    }
    fn visit_item_enum_mut(&mut self, i: &mut syn::ItemEnum) {
        i.attrs.retain(|attr| !attr.path().is_ident("doc"));
        for variant in &mut i.variants {
            variant.attrs.retain(|attr| !attr.path().is_ident("doc"));
        }
        if let Some(selected) = self.defaults.get(&i.ident.to_string()) {
            i.attrs.push(syn::parse_quote!(#[derive(Default)]));
            for variant in i
                .variants
                .iter_mut()
                .filter(|variant| variant.ident == selected.as_str())
            {
                variant.attrs.push(syn::parse_quote!(#[default]));
            }
        }
        syn::visit_mut::visit_item_enum_mut(self, i);
    }
}
