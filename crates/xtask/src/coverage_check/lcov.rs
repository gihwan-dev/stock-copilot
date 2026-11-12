use std::collections::BTreeMap;
use std::path::Path;

use crate::workspace_sources::relative_text;

pub(crate) type LineCounts = BTreeMap<usize, u64>;

fn line_record(record: &str) -> Option<(usize, u64)> {
    let mut fields = record.split(',');
    let line = fields.next()?.parse().ok()?;
    let count = fields.next()?.parse().ok()?;
    Some((line, count))
}

pub(crate) fn parse_lcov(text: &str, root: &Path) -> BTreeMap<String, LineCounts> {
    let mut files: BTreeMap<String, LineCounts> = BTreeMap::new();
    let mut current: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(source) = line.strip_prefix("SF:") {
            let relative = relative_text(root, Path::new(source));
            files.entry(relative.clone()).or_default();
            current = Some(relative);
        } else if line == "end_of_record" {
            current = None;
        } else if let (Some(record), Some(path)) = (line.strip_prefix("DA:"), current.as_ref())
            && let Some((number, count)) = line_record(record)
        {
            let slot = files
                .entry(path.clone())
                .or_default()
                .entry(number)
                .or_insert(0);
            *slot = slot.saturating_add(count);
        }
    }
    files
}
