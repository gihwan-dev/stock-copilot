use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::capability_check::json::{Json, parse};
use crate::workspace_sources::relative_text;

const CODE_REGION: &str = "0";
const START_LINE: usize = 0;
const START_COLUMN: usize = 1;
const END_LINE: usize = 2;
const END_COLUMN: usize = 3;
const COUNT: usize = 4;
const FILE: usize = 5;
const KIND: usize = 7;

pub(crate) type RegionKey = (usize, usize, usize, usize);

pub(crate) type Regions = BTreeMap<String, BTreeMap<RegionKey, bool>>;

fn number(value: Option<&Json>) -> Option<&str> {
    match value? {
        Json::Number(text) => Some(text.as_str()),
        Json::Null
        | Json::True
        | Json::False
        | Json::Text(_)
        | Json::Array(_)
        | Json::Object(_) => None,
    }
}

fn index(value: Option<&Json>) -> Option<usize> {
    number(value)?.parse().ok()
}

fn record_region(fields: &[Json], files: &[String], regions: &mut Regions) -> Option<()> {
    if number(fields.get(KIND))? != CODE_REGION {
        return Some(());
    }
    let file = files.get(index(fields.get(FILE))?)?;
    let key = (
        index(fields.get(START_LINE))?,
        index(fields.get(START_COLUMN))?,
        index(fields.get(END_LINE))?,
        index(fields.get(END_COLUMN))?,
    );
    let executed = number(fields.get(COUNT))? != "0";
    let slot = regions
        .entry(file.clone())
        .or_default()
        .entry(key)
        .or_insert(false);
    *slot = *slot || executed;
    Some(())
}

pub(crate) fn parse_regions(text: &str, root: &Path) -> Option<Regions> {
    let document = parse(text)?;
    let mut regions = Regions::new();
    for data in document.get("data")?.as_array()? {
        for function in data.get("functions")?.as_array()? {
            let files: Vec<String> = function
                .get("filenames")?
                .as_array()?
                .iter()
                .map(|name| {
                    name.as_str()
                        .map(|path| relative_text(root, Path::new(path)))
                })
                .collect::<Option<Vec<String>>>()?;
            for region in function.get("regions")?.as_array()? {
                record_region(region.as_array()?, &files, &mut regions)?;
            }
        }
    }
    Some(regions)
}

pub(crate) fn missed_region_lines(regions: &BTreeMap<RegionKey, bool>) -> BTreeSet<usize> {
    regions
        .iter()
        .filter(|(_, executed)| !**executed)
        .map(|((line, _, _, _), _)| *line)
        .collect()
}
