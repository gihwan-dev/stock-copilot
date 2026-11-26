use domain::{FundKind, KnownMarket, ListingStatus};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::IpcError;
use super::simulation::{UiFundKind, ui_kind};
use crate::holdings_link::{DirectoryEntry, InstrumentDirectory};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SearchInstrumentsRequest {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub fund_kind: Option<UiFundKindFilter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum UiFundKindFilter {
    #[serde(rename = "single-security")]
    SingleSecurity,
    #[serde(rename = "broad-index-fund")]
    BroadIndexFund,
    #[serde(rename = "leveraged-or-inverse")]
    LeveragedOrInverse,
    #[serde(rename = "sector-or-theme-fund")]
    SectorOrThemeFund,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentMatch {
    pub symbol: String,
    pub market: String,
    pub name: String,
    pub fund_kind: UiFundKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentSearch {
    pub matches: Vec<UiInstrumentMatch>,
}

const fn wanted(filter: UiFundKindFilter) -> FundKind {
    match filter {
        UiFundKindFilter::SingleSecurity => FundKind::SingleSecurity,
        UiFundKindFilter::BroadIndexFund => FundKind::BroadIndexFund,
        UiFundKindFilter::LeveragedOrInverse => FundKind::LeveragedOrInverse,
        UiFundKindFilter::SectorOrThemeFund => FundKind::SectorOrThemeFund,
    }
}

fn matches(entry: &DirectoryEntry, query: Option<&str>, filter: Option<FundKind>) -> bool {
    let kind = entry.master.fund_kind();
    let listed = entry.master.row().listing_status == ListingStatus::Active;
    let by_kind = filter.is_none_or(|expected| expected == kind);
    let by_text = query.is_none_or(|text| {
        let needle = text.to_uppercase().replace(' ', "");
        let name = entry.name.as_str().to_uppercase().replace(' ', "");
        name.contains(&needle) || entry.master.row().symbol.as_str() == text.to_uppercase()
    });
    listed && by_kind && by_text
}

pub(crate) fn search(
    directory: &InstrumentDirectory,
    request: &SearchInstrumentsRequest,
) -> UiInstrumentSearch {
    let query = request
        .query
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let filter = request.fund_kind.map(wanted);
    let mut found: Vec<UiInstrumentMatch> = directory
        .entries()
        .iter()
        .filter(|entry| matches(entry, query, filter))
        .map(|entry| UiInstrumentMatch {
            symbol: entry.master.row().symbol.as_str().to_owned(),
            market: String::from(match entry.market {
                KnownMarket::Kr => "KR",
                KnownMarket::Us => "US",
            }),
            name: entry.name.as_str().to_owned(),
            fund_kind: ui_kind(entry.master.fund_kind()),
        })
        .collect();
    found.sort_by(|left, right| left.name.cmp(&right.name));
    found.truncate(20);
    UiInstrumentSearch { matches: found }
}

#[tauri::command]
#[specta::specta]
pub async fn search_instruments<R: Runtime>(
    app: AppHandle<R>,
    request: SearchInstrumentsRequest,
) -> Result<UiInstrumentSearch, IpcError> {
    let directory = app
        .try_state::<InstrumentDirectory>()
        .map_or_else(InstrumentDirectory::default, |state| state.inner().clone());
    Ok(search(&directory, &request))
}
