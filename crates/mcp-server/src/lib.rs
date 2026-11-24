mod handler;
mod peer;
mod server;
mod socket;
mod token;

pub use handler::{ConnectionEvent, ConnectionRejection};
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
