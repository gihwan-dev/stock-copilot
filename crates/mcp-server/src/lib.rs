pub mod catalog;
mod handler;
mod pagination;
mod peer;
mod server;
mod socket;
mod token;
mod tools;
mod untrusted;

#[cfg(test)]
mod contract_tests;

pub use catalog::market::register_market_tools;
pub use catalog::portfolio::register_portfolio_tools;
pub use handler::{ConnectionEvent, ConnectionRejection};
pub use pagination::{
    CursorStore, DEFAULT_CURSOR_TTL_SECS, DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT, MAX_STORED_CURSORS,
    MAX_TOOL_RESPONSE_CHARS, OpaqueCursor, PageLimit, PageRequest, PaginationContext,
    paginate_items,
};
pub use peer::{PeerInspectorMode, PeerVerificationFailure, UserId, VerifiedPeer};
pub use server::{
    DEFAULT_AUTH_TIMEOUT_SECS, DEFAULT_MAX_CONCURRENT_CONNECTIONS,
    DEFAULT_PEER_INSPECTION_TIMEOUT_SECS, MAX_AUTH_TIMEOUT_SECS, MAX_CONCURRENT_CONNECTIONS_CAP,
    MAX_PEER_INSPECTION_TIMEOUT_SECS, MaxConcurrentConnections, McpServerConfig, McpServerError,
    McpServerHandle, PeerProcessTarget, RunIdentity, RunServerDeps, RunServerRequest,
};
pub use socket::{
    DEFAULT_SOCKET_FILE_NAME, DIRECTORY_MODE, MAX_UNIX_SOCKET_PATH_BYTES, RunSocketSpec,
    SOCKET_MODE, SocketSetupError, validate_socket_path_length,
};
pub use token::{
    AUTH_LINE_PREFIX, AuthLineLimit, AuthRejection, DEFAULT_MAX_AUTH_LINE_BYTES,
    DEFAULT_TOKEN_TTL_SECONDS, MAX_AUTH_LINE_BYTES_CAP, MAX_TOKEN_TTL_SECONDS, MIN_AUTH_LINE_BYTES,
    RunAuthToken, TOKEN_HEX_BYTES, TOKEN_RANDOM_BYTES, TokenError, TokenTtl,
};
pub use tools::{
    MAX_TOOL_ERROR_CHARS, McpTool, RetryPolicy, ToolAccessKind, ToolErrorCode, ToolErrorPayload,
    ToolFuture, ToolInvocation, ToolLedgerMode, ToolRegistry, ToolRegistryError, ToolSpec,
    UnconfiguredToolLedger, build_object_schema, make_cursor_store,
    validate_arguments_against_schema,
};
pub use untrusted::{
    DEFAULT_UNTRUSTED_TEXT_CHARS, MAX_UNTRUSTED_TEXT_CHARS, NormalizedUntrustedText, Truncation,
    UNTRUSTED_TEXT_FIELD, UntrustedTextLimit, UntrustedTextLimitError, is_removed_char,
    untrusted_text_value,
};
