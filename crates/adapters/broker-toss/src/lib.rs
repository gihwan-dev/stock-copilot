mod error_codes;
mod error_mapping;
mod time;
pub use error_codes::{ApiErrorCode, OAuthErrorCode};
pub use error_mapping::{
    CodeRecognition, CursorHint, DiagnosticId, DiagnosticIds, TossErrorBody, TossMappedError,
    map_toss_read_error, map_toss_token_error,
};
pub use time::{TossDate, TossTimeError, TossTimestamp};
mod endpoint;
mod http;
mod request;
mod token;
pub use endpoint::{TossEndpoint, TossWsEndpoint};
pub use http::{
    PublicDocsRoute, RawResponse, ResponseHeaders, TossHttpClient, TossHttpError, TossWsConnector,
};
pub use request::{QueryKey, ReadRequest, ReadRoute, RouteId};
pub use token::issue_token;

mod rate_limiter;
pub use rate_limiter::{RequestWait, TokioRequestWait};
mod token_manager;
pub use token_manager::{BrokerConnection, TokenError, TokenLease, TokenManager, TokenRenewal};

mod account_read;
mod holding_read;
pub use account_read::TossAccountRead;
mod account_ports;
mod transaction_read;

mod market_read;

mod instrument_master;
mod instrument_ports;
mod market_calendar;
mod market_flow;
mod market_fx;
mod market_indicator_candles;
mod market_indicators;
mod market_info;
mod market_investor_rankings;
mod market_pages;
mod market_rankings;
mod market_sector;
mod market_sector_rankings;
mod market_supply;
pub use instrument_ports::TossInstrumentRead;
mod instrument_market;
mod order_info_read;
mod order_read;
mod sha256;
mod spec_check;
pub use sha256::{sha256_digest, sha256_hex};
pub use spec_check::{
    PINNED_LAST_MODIFIED, PINNED_NORMALIZED_SPEC_SHA256, PINNED_SPEC_VERSION, TossSpecClient,
    normalized_json_sha256, pinned_spec_baseline,
};
mod ws;
mod ws_frame;
mod ws_protocol;
pub use ws::{
    TossWsClient, TossWsDeps, WsAccountSlotPool, WsClientError, WsConfig, WsDisconnectCause,
    WsDispatchedEvent, WsReconcileReport, compute_backoff,
};
pub use ws_protocol::{
    MAX_TOPICS_PER_CONNECTION, WsChannel, WsErrorCode, WsOrderEventKind, WsOrderNotice,
    WsOrderbookTick, WsProtocolError, WsRejectedCode, WsRejectedTopic, WsServerEvent,
    WsSubscriptionSet, WsTopic, WsTradeTick, decode_server_text,
};
