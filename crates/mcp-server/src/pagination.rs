use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use application::{AiRunId, Clock, ToolName};
use domain::{Nanosecond, SpanSeconds, TimeSpan, Timestamp};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::tools::{ToolErrorCode, ToolErrorPayload};

pub const MAX_TOOL_RESPONSE_CHARS: usize = 30_000_usize;
pub const DEFAULT_PAGE_LIMIT: usize = 50_usize;
pub const MAX_PAGE_LIMIT: usize = 200_usize;
pub const DEFAULT_CURSOR_TTL_SECS: u64 = 600_u64;
pub const MAX_STORED_CURSORS: usize = 128_usize;

const CURSOR_RANDOM_BYTES: usize = 16_usize;
const CURSOR_HEX_BYTES: usize = 32_usize;
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLimit(usize);

impl PageLimit {
    pub fn try_new(limit: usize) -> Result<Self, ToolErrorPayload> {
        if !(1_usize..=MAX_PAGE_LIMIT).contains(&limit) {
            return Err(ToolErrorPayload::invalid_input(
                Some("limit"),
                "limit must be between 1 and 200",
            ));
        }
        Ok(Self(limit))
    }

    #[must_use]
    pub const fn default_limit() -> Self {
        Self(DEFAULT_PAGE_LIMIT)
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpaqueCursor(String);

impl OpaqueCursor {
    pub fn try_parse(raw: &str) -> Result<Self, ToolErrorPayload> {
        let bytes = raw.as_bytes();
        if bytes.len() != CURSOR_HEX_BYTES
            || !bytes
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(ToolErrorPayload::non_retryable(
                ToolErrorCode::InvalidCursor,
                Some("cursor"),
                "cursor handle format is invalid",
            ));
        }
        Ok(Self(raw.to_owned()))
    }

    fn generate() -> Result<Self, ToolErrorPayload> {
        let mut raw = [0_u8; CURSOR_RANDOM_BYTES];
        getrandom::fill(&mut raw).map_err(|_error| {
            ToolErrorPayload::retryable(
                ToolErrorCode::InternalError,
                None,
                "failed to allocate cursor handle",
            )
        })?;
        let mut text = String::with_capacity(CURSOR_HEX_BYTES);
        for byte in raw {
            let high = usize::from(byte.wrapping_shr(4_u32));
            let low = usize::from(byte & 0x0f_u8);
            let Some(&high_char) = HEX_DIGITS.get(high) else {
                return Err(ToolErrorPayload::retryable(
                    ToolErrorCode::InternalError,
                    None,
                    "failed to encode cursor handle",
                ));
            };
            let Some(&low_char) = HEX_DIGITS.get(low) else {
                return Err(ToolErrorPayload::retryable(
                    ToolErrorCode::InternalError,
                    None,
                    "failed to encode cursor handle",
                ));
            };
            text.push(char::from(high_char));
            text.push(char::from(low_char));
        }
        Ok(Self(text))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRequest {
    limit: PageLimit,
    cursor: Option<OpaqueCursor>,
}

impl PageRequest {
    pub fn from_raw(
        raw_limit: Option<u64>,
        raw_cursor: Option<&str>,
    ) -> Result<Self, ToolErrorPayload> {
        let limit = match raw_limit {
            Some(value) => {
                let parsed = usize::try_from(value).map_err(|_error| {
                    ToolErrorPayload::invalid_input(
                        Some("limit"),
                        "limit must be between 1 and 200",
                    )
                })?;
                PageLimit::try_new(parsed)?
            }
            None => PageLimit::default_limit(),
        };
        let cursor = match raw_cursor {
            Some(text) => Some(OpaqueCursor::try_parse(text)?),
            None => None,
        };
        Ok(Self { limit, cursor })
    }

    #[must_use]
    pub const fn limit(&self) -> PageLimit {
        self.limit
    }

    #[must_use]
    pub const fn cursor(&self) -> Option<&OpaqueCursor> {
        self.cursor.as_ref()
    }
}

#[derive(Debug, Clone)]
struct CursorEntry {
    handle: OpaqueCursor,
    run_id: AiRunId,
    tool_name: ToolName,
    query_fingerprint: String,
    offset: usize,
    expires_at: Timestamp,
}

pub struct CursorStore<C: Clock> {
    clock: Arc<C>,
    ttl: TimeSpan,
    entries: RwLock<VecDeque<CursorEntry>>,
}

impl<C: Clock> std::fmt::Debug for CursorStore<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = match self.entries.read() {
            Ok(locked) => locked.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        };
        f.debug_struct("CursorStore")
            .field("ttl", &self.ttl)
            .field("entry_count", &count)
            .finish_non_exhaustive()
    }
}

impl<C: Clock> CursorStore<C> {
    pub fn try_new(clock: Arc<C>, ttl_secs: u64) -> Result<Self, ToolErrorPayload> {
        let ttl = TimeSpan::try_new(SpanSeconds::new(ttl_secs), Nanosecond::new(0_u32)).map_err(
            |_error| {
                ToolErrorPayload::non_retryable(
                    ToolErrorCode::InternalError,
                    None,
                    "invalid cursor ttl",
                )
            },
        )?;
        Ok(Self {
            clock,
            ttl,
            entries: RwLock::new(VecDeque::new()),
        })
    }

    #[must_use]
    pub fn with_default_ttl(clock: Arc<C>) -> Self {
        Self {
            clock,
            ttl: TimeSpan::DAY,
            entries: RwLock::new(VecDeque::new()),
        }
    }

    pub fn resolve(
        &self,
        cursor: &OpaqueCursor,
        run_id: &AiRunId,
        tool_name: &ToolName,
        query_fingerprint: &str,
    ) -> Result<usize, ToolErrorPayload> {
        let now = self.clock.now();
        let guard = match self.entries.read() {
            Ok(locked) => locked,
            Err(poisoned) => poisoned.into_inner(),
        };
        let Some(found) = guard.iter().find(|entry| &entry.handle == cursor) else {
            return Err(ToolErrorPayload::non_retryable(
                ToolErrorCode::InvalidCursor,
                Some("cursor"),
                "cursor handle is unknown",
            ));
        };
        if &found.run_id != run_id
            || &found.tool_name != tool_name
            || found.query_fingerprint != query_fingerprint
        {
            return Err(ToolErrorPayload::non_retryable(
                ToolErrorCode::InvalidCursor,
                Some("cursor"),
                "cursor handle does not match this run or query",
            ));
        }
        if now >= found.expires_at {
            return Err(ToolErrorPayload::non_retryable(
                ToolErrorCode::CursorExpired,
                Some("cursor"),
                "cursor handle has expired",
            ));
        }
        Ok(found.offset)
    }

    pub fn issue(
        &self,
        run_id: &AiRunId,
        tool_name: &ToolName,
        query_fingerprint: &str,
        offset: usize,
    ) -> Result<OpaqueCursor, ToolErrorPayload> {
        let now = self.clock.now();
        let expires_at = now.checked_add(self.ttl).map_err(|_error| {
            ToolErrorPayload::retryable(
                ToolErrorCode::InternalError,
                None,
                "cursor expiry timestamp overflowed",
            )
        })?;
        let handle = OpaqueCursor::generate()?;
        let mut guard = match self.entries.write() {
            Ok(locked) => locked,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.retain(|entry| now < entry.expires_at);
        if guard.len() >= MAX_STORED_CURSORS {
            let _evicted = guard.pop_front();
        }
        guard.push_back(CursorEntry {
            handle: handle.clone(),
            run_id: run_id.clone(),
            tool_name: tool_name.clone(),
            query_fingerprint: query_fingerprint.to_owned(),
            offset,
            expires_at,
        });
        Ok(handle)
    }
}

#[derive(Debug)]
pub struct PaginationContext<'a, C: Clock> {
    pub cursors: &'a CursorStore<C>,
    pub run_id: &'a AiRunId,
    pub tool_name: &'a ToolName,
    pub query_fingerprint: &'a str,
}

fn build_page_json<T: Serialize>(
    slice: &[T],
    total: usize,
    next_cursor: Option<&str>,
    extra_fields: &Map<String, Value>,
) -> Result<(Value, usize), ToolErrorPayload> {
    let mut object = extra_fields.clone();
    let encoded_items = serde_json::to_value(slice).map_err(|_error| {
        ToolErrorPayload::retryable(
            ToolErrorCode::InternalError,
            None,
            "failed to serialize page items",
        )
    })?;
    object.insert(String::from("items"), encoded_items);
    object.insert(String::from("total"), Value::from(total));
    object.insert(String::from("returned"), Value::from(slice.len()));
    if let Some(cursor) = next_cursor {
        object.insert(
            String::from("next_cursor"),
            Value::String(cursor.to_owned()),
        );
    } else {
        object.insert(String::from("next_cursor"), Value::Null);
    }
    let value = Value::Object(object);
    let rendered = serde_json::to_string(&value).map_err(|_error| {
        ToolErrorPayload::retryable(
            ToolErrorCode::InternalError,
            None,
            "failed to encode page response",
        )
    })?;
    Ok((value, rendered.chars().count()))
}

fn finalize_fitted_page<T: Serialize, C: Clock>(
    slice: &[T],
    total: usize,
    end: usize,
    context: &PaginationContext<'_, C>,
    extra_fields: &Map<String, Value>,
) -> Result<Value, ToolErrorPayload> {
    let actual_cursor = if end < total {
        Some(context.cursors.issue(
            context.run_id,
            context.tool_name,
            context.query_fingerprint,
            end,
        )?)
    } else {
        None
    };
    let (final_value, final_chars) = build_page_json(
        slice,
        total,
        actual_cursor.as_ref().map(OpaqueCursor::as_str),
        extra_fields,
    )?;
    if final_chars > MAX_TOOL_RESPONSE_CHARS {
        return Err(ToolErrorPayload::response_too_large());
    }
    Ok(final_value)
}

pub fn paginate_items<T: Serialize, C: Clock>(
    items: &[T],
    request: &PageRequest,
    context: &PaginationContext<'_, C>,
    extra_fields: &Map<String, Value>,
) -> Result<Value, ToolErrorPayload> {
    let total = items.len();
    let start = match request.cursor() {
        Some(cursor) => context.cursors.resolve(
            cursor,
            context.run_id,
            context.tool_name,
            context.query_fingerprint,
        )?,
        None => 0_usize,
    };
    if start >= total {
        let (empty_value, char_count) = build_page_json::<T>(&[], total, None, extra_fields)?;
        if char_count > MAX_TOOL_RESPONSE_CHARS {
            return Err(ToolErrorPayload::response_too_large());
        }
        return Ok(empty_value);
    }

    let max_end = start.saturating_add(request.limit().as_usize()).min(total);
    let mut count = max_end.saturating_sub(start);
    let placeholder_cursor = "0".repeat(CURSOR_HEX_BYTES);

    loop {
        let end = start.saturating_add(count);
        let Some(slice) = items.get(start..end) else {
            return Err(ToolErrorPayload::non_retryable(
                ToolErrorCode::InvalidCursor,
                Some("cursor"),
                "cursor offset is out of range",
            ));
        };
        let check_cursor = (end < total).then_some(placeholder_cursor.as_str());
        let (_candidate, char_count) = build_page_json(slice, total, check_cursor, extra_fields)?;
        if char_count <= MAX_TOOL_RESPONSE_CHARS {
            return finalize_fitted_page(slice, total, end, context, extra_fields);
        }
        if count <= 1_usize {
            return Err(ToolErrorPayload::response_too_large());
        }
        count = count.saturating_sub(1_usize).max(1_usize);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{AiRunId, ToolName};
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use serde_json::Map;
    use testkit_ports::SteppedClock;

    use crate::tools::ToolErrorCode;

    use super::{
        CursorStore, MAX_PAGE_LIMIT, MAX_TOOL_RESPONSE_CHARS, OpaqueCursor, PageLimit, PageRequest,
        PaginationContext, paginate_items,
    };

    fn ts(secs: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(secs),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("시각")
    }

    #[test]
    fn page_limit_and_opaque_cursor_validate_bounds() {
        assert_eq!(
            PageLimit::try_new(0_usize).err().map(|err| err.code()),
            Some(ToolErrorCode::InvalidInput)
        );
        assert_eq!(
            PageLimit::try_new(MAX_PAGE_LIMIT.saturating_add(1_usize))
                .err()
                .map(|err| err.code()),
            Some(ToolErrorCode::InvalidInput)
        );
        assert_eq!(
            PageLimit::try_new(10_usize).map(PageLimit::as_usize),
            Ok(10)
        );
        assert_eq!(
            OpaqueCursor::try_parse("bad").err().map(|err| err.code()),
            Some(ToolErrorCode::InvalidCursor)
        );
    }

    #[test]
    fn paginate_items_enforces_cursor_binding_expiry_and_30k_char_cap() {
        let clock = Arc::new(SteppedClock::new(ts(1_700_000_000_i64)));
        let store = CursorStore::try_new(Arc::clone(&clock), 600_u64).expect("커서 저장소");
        let run_a = AiRunId::try_new("run-a").expect("run a");
        let run_b = AiRunId::try_new("run-b").expect("run b");
        let tool = ToolName::try_new("list_items").expect("tool");
        let items: Vec<String> = (0_usize..5_usize)
            .map(|idx| format!("item-{idx}-{}", "x".repeat(12_000_usize)))
            .collect();
        let req = PageRequest::from_raw(Some(5_u64), None).expect("req");
        let ctx_a = PaginationContext {
            cursors: &store,
            run_id: &run_a,
            tool_name: &tool,
            query_fingerprint: "q=1",
        };
        let page1 = paginate_items(&items, &req, &ctx_a, &Map::new()).expect("1페이지");
        let rendered1 = serde_json::to_string(&page1).expect("직렬화");
        assert!(
            rendered1.chars().count() <= MAX_TOOL_RESPONSE_CHARS,
            "페이지 응답은 3만 자 이하"
        );
        assert_eq!(
            page1.get("returned").and_then(serde_json::Value::as_u64),
            Some(2)
        );
        let next_handle = page1
            .get("next_cursor")
            .and_then(serde_json::Value::as_str)
            .expect("다음 커서");

        let req2 = PageRequest::from_raw(Some(5_u64), Some(next_handle)).expect("req2");
        let ctx_b = PaginationContext {
            cursors: &store,
            run_id: &run_b,
            tool_name: &tool,
            query_fingerprint: "q=1",
        };
        assert_eq!(
            paginate_items(&items, &req2, &ctx_b, &Map::new())
                .err()
                .map(|err| err.code()),
            Some(ToolErrorCode::InvalidCursor),
            "다른 실행 ID의 커서 거부"
        );

        let page2 = paginate_items(&items, &req2, &ctx_a, &Map::new()).expect("2페이지");
        assert_eq!(
            page2.get("returned").and_then(serde_json::Value::as_u64),
            Some(2)
        );

        clock
            .advance_to(ts(1_700_000_601_i64))
            .expect("만료 시각 이동");
        assert_eq!(
            paginate_items(&items, &req2, &ctx_a, &Map::new())
                .err()
                .map(|err| err.code()),
            Some(ToolErrorCode::CursorExpired),
            "만료된 커서 거부"
        );
    }
}
