mod connection;
pub(crate) mod migration;
pub(crate) use connection::{
    CIPHER_HMAC, CIPHER_INTEGRITY, CIPHER_MEMORY, FOREIGN_KEYS, INCREMENTAL_VACUUM, JOURNAL,
    PAGE_SIZE, PROVIDER, QUERY_ONLY, QUICK_CHECK, SCHEMA, SET_AUTO_VACUUM, SET_CIPHER_MEMORY,
    SET_FOREIGN_KEYS, SET_JOURNAL, SET_JOURNAL_DELETE, SET_QUERY_ONLY, SET_SYNCHRONOUS,
    SET_TEMP_STORE, SYNCHRONOUS, TEMP_STORE,
};
#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
pub(crate) mod review_tests;

#[cfg(test)]
pub(crate) mod third_tests;

pub(crate) mod ai_preferences;
pub(crate) mod holdings;
#[cfg(test)]
pub(crate) mod holdings_tests;
pub(crate) mod master;

pub(crate) mod market;
pub(crate) mod transactions;
#[cfg(test)]
pub(crate) mod transactions_tests;

#[cfg(test)]
pub(crate) mod market_tests;

#[cfg(test)]
pub(crate) mod fts_guard_tests;

#[cfg(test)]
pub(crate) mod information_tests;

pub(crate) mod information;

pub(crate) mod thesis;
#[cfg(test)]
pub(crate) mod thesis_tests;

pub(crate) mod proposal;
#[cfg(test)]
pub(crate) mod proposal_tests;

pub(crate) mod instrument_mapping;
#[cfg(test)]
pub(crate) mod instrument_mapping_history_tests;
#[cfg(test)]
pub(crate) mod instrument_mapping_tests;

pub(crate) mod estimates;
#[cfg(test)]
pub(crate) mod estimates_tests;

pub(crate) mod signals;
#[cfg(test)]
pub(crate) mod signals_tests;

pub(crate) mod audit;
#[cfg(test)]
pub(crate) mod audit_tests;

pub(crate) mod chat;
#[cfg(test)]
pub(crate) mod chat_tests;

pub(crate) mod settings;
#[cfg(test)]
pub(crate) mod settings_tests;

pub(crate) mod order_rate;
#[cfg(test)]
pub(crate) mod order_rate_tests;

pub(crate) mod status;
#[cfg(test)]
pub(crate) mod status_tests;

pub(crate) mod wake;
#[cfg(test)]
pub(crate) mod wake_tests;

pub(crate) mod watchlist;
#[cfg(test)]
pub(crate) mod watchlist_tests;

pub(crate) mod briefing;
#[cfg(test)]
pub(crate) mod briefing_tests;
pub(crate) mod dashboard_research;
pub(crate) mod jobs;

#[cfg(test)]
pub(crate) mod close_evidence_tests;

pub(crate) mod ai_jobs;
#[cfg(test)]
pub(crate) mod ai_jobs_tests;
