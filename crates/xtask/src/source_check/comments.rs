use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

pub(crate) fn comment_lines(source: &str) -> Vec<usize> {
    let mut line = 1_usize;
    let mut rest = source;
    let mut found = Vec::new();
    for token in tokenize(source, FrontmatterAllowed::No) {
        let Ok(length) = usize::try_from(token.len) else {
            break;
        };
        let Some((text, remaining)) = rest.split_at_checked(length) else {
            break;
        };
        if let TokenKind::LineComment { .. } | TokenKind::BlockComment { .. } = token.kind {
            found.push(line);
        }
        line = line.saturating_add(text.matches('\n').count());
        rest = remaining;
    }
    found
}
