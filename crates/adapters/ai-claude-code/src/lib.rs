mod decoder;
mod error;
mod isolation;
mod parse;
mod process;
mod provider;

pub use decoder::{ClaudeEventDecoder, decode_stream_lines, decode_stream_text};
pub use error::{FieldName, SchemaFailure, StreamDecodeError, StreamOrderError, ValidationFailure};
pub use isolation::IsolationExpectation;
pub use process::{MAX_STREAM_LINE_BYTES, ProcessSignal, send_signal_to_process_group};
pub use provider::{ClaudeCodeConfig, ClaudeCodeProvider, ClaudeEventStream, ModelDefaults};
