use proc_macro2::{TokenStream, TokenTree};

const MACROS: &[&str] = &["line", "file", "module_path", "column"];

pub(super) fn sensitive(tokens: TokenStream) -> bool {
    let values: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, token) in values.iter().enumerate() {
        if let TokenTree::Group(group) = token
            && sensitive(group.stream())
        {
            return true;
        }
        if attribute(&values, index) {
            return true;
        }
        let TokenTree::Ident(identifier) = token else {
            continue;
        };
        let name = crate::rust_names::ident(identifier);
        if MACROS.contains(&name.as_str())
            && punct(values.get(index.saturating_add(1)), '!')
            && group(values.get(index.saturating_add(2)))
        {
            return true;
        }
        if name == "Location"
            && ((punct(index.checked_sub(1).and_then(|at| values.get(at)), ':')
                && punct(index.checked_sub(2).and_then(|at| values.get(at)), ':'))
                || (punct(values.get(index.saturating_add(1)), ':')
                    && punct(values.get(index.saturating_add(2)), ':')))
        {
            return true;
        }
        if name == "use" && values.get(index..).is_none_or(import) {
            return true;
        }
    }
    false
}

fn punct(token: Option<&TokenTree>, value: char) -> bool {
    token.is_some_and(|next| {
        if let TokenTree::Punct(punctuation) = next {
            punctuation.as_char() == value
        } else {
            false
        }
    })
}

fn import(tokens: &[TokenTree]) -> bool {
    let ending = tokens.iter().position(|token| punct(Some(token), ';'));
    let Some(end) = ending else {
        return true;
    };
    let source = tokens.iter().take(end.saturating_add(1)).cloned().collect();
    syn::parse2::<syn::ItemUse>(source).map_or(true, |item| import_tree(&item.tree, &[]))
}

fn import_tree(tree: &syn::UseTree, prefix: &[String]) -> bool {
    match tree {
        syn::UseTree::Path(path) => {
            let mut names = prefix.to_vec();
            names.push(crate::rust_names::ident(&path.ident));
            import_tree(&path.tree, &names)
        }
        syn::UseTree::Group(group) => group.items.iter().any(|child| import_tree(child, prefix)),
        syn::UseTree::Name(name) => imported(prefix, &crate::rust_names::ident(&name.ident)),
        syn::UseTree::Rename(name) => imported(prefix, &crate::rust_names::ident(&name.ident)),
        syn::UseTree::Glob(_) => prefix
            .iter()
            .any(|name| ["std", "core"].contains(&name.as_str())),
    }
}

fn imported(prefix: &[String], last: &str) -> bool {
    MACROS.contains(&last)
        || ["Location", "track_caller"].contains(&last)
        || (prefix
            .first()
            .is_some_and(|name| ["std", "core"].contains(&name.as_str()))
            && (last == "panic"
                || prefix.iter().any(|name| name == "panic")
                || (last == "self" && prefix.len() == 1)))
        || (prefix.is_empty() && ["std", "core"].contains(&last))
}

fn group(token: Option<&TokenTree>) -> bool {
    token.is_some_and(|next| match next {
        TokenTree::Group(_) => true,
        TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Punct(_) => false,
    })
}

fn attribute(tokens: &[TokenTree], index: usize) -> bool {
    if !punct(tokens.get(index), '#') {
        return false;
    }
    let offset = if punct(tokens.get(index.saturating_add(1)), '!') {
        2
    } else {
        1
    };
    let Some(TokenTree::Group(group)) = tokens.get(index.saturating_add(offset)) else {
        return true;
    };
    syn::parse2::<syn::Meta>(group.stream()).map_or(true, |meta| {
        meta.path()
            .segments
            .last()
            .is_some_and(|segment| crate::rust_names::ident(&segment.ident) == "track_caller")
    })
}
