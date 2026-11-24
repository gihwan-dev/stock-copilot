use std::future::Future;

use domain::Timestamp;

use crate::{AiRunId, RecordOutcome, SystemPortError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolLedgerError {
    ToolNameInvalid,
    RequestIdMalformed,
    PayloadBounds,
    ProtocolVersionSyntax,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolName(String);

impl ToolName {
    pub fn try_new(raw: &str) -> Result<Self, ToolLedgerError> {
        let bytes = raw.as_bytes();
        if bytes.is_empty() || bytes.len() > 64_usize {
            return Err(ToolLedgerError::ToolNameInvalid);
        }
        if !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(byte))
        {
            return Err(ToolLedgerError::ToolNameInvalid);
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolRequestId(String);

impl ToolRequestId {
    pub fn try_new(raw: &str) -> Result<Self, ToolLedgerError> {
        let bytes = raw.as_bytes();
        if bytes.is_empty() || bytes.len() > 128_usize {
            return Err(ToolLedgerError::RequestIdMalformed);
        }
        if !bytes.iter().all(|byte| (0x20_u8..=0x7e_u8).contains(byte)) {
            return Err(ToolLedgerError::RequestIdMalformed);
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPayloadJson(String);

impl ToolPayloadJson {
    pub fn try_new(raw: &str) -> Result<Self, ToolLedgerError> {
        let bytes = raw.as_bytes();
        if bytes.is_empty() || bytes.len() > 65_536_usize {
            return Err(ToolLedgerError::PayloadBounds);
        }
        if bytes.contains(&0_u8) {
            return Err(ToolLedgerError::PayloadBounds);
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProtocolVersion(String);

impl ToolProtocolVersion {
    pub fn try_new(raw: &str) -> Result<Self, ToolLedgerError> {
        let bytes = raw.as_bytes();
        if bytes.is_empty() || bytes.len() > 32_usize {
            return Err(ToolLedgerError::ProtocolVersionSyntax);
        }
        if !bytes.iter().all(|byte| (0x21_u8..=0x7e_u8).contains(byte)) {
            return Err(ToolLedgerError::ProtocolVersionSyntax);
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallOutcome {
    Succeeded,
    ToolError,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallRecord {
    run_id: AiRunId,
    request_id: ToolRequestId,
    tool_name: ToolName,
    arguments_json: ToolPayloadJson,
    result_json: ToolPayloadJson,
    outcome: ToolCallOutcome,
    started_at: Timestamp,
    finished_at: Timestamp,
    protocol_version: ToolProtocolVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallRecordInput {
    pub run_id: AiRunId,
    pub request_id: ToolRequestId,
    pub tool_name: ToolName,
    pub arguments_json: ToolPayloadJson,
    pub result_json: ToolPayloadJson,
    pub outcome: ToolCallOutcome,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub protocol_version: ToolProtocolVersion,
}

impl ToolCallRecord {
    #[must_use]
    pub fn new(input: ToolCallRecordInput) -> Self {
        Self {
            run_id: input.run_id,
            request_id: input.request_id,
            tool_name: input.tool_name,
            arguments_json: input.arguments_json,
            result_json: input.result_json,
            outcome: input.outcome,
            started_at: input.started_at,
            finished_at: input.finished_at,
            protocol_version: input.protocol_version,
        }
    }

    #[must_use]
    pub const fn run_id(&self) -> &AiRunId {
        &self.run_id
    }

    #[must_use]
    pub const fn request_id(&self) -> &ToolRequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn tool_name(&self) -> &ToolName {
        &self.tool_name
    }

    #[must_use]
    pub const fn arguments_json(&self) -> &ToolPayloadJson {
        &self.arguments_json
    }

    #[must_use]
    pub const fn result_json(&self) -> &ToolPayloadJson {
        &self.result_json
    }

    #[must_use]
    pub const fn outcome(&self) -> ToolCallOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn started_at(&self) -> Timestamp {
        self.started_at
    }

    #[must_use]
    pub const fn finished_at(&self) -> Timestamp {
        self.finished_at
    }

    #[must_use]
    pub const fn protocol_version(&self) -> &ToolProtocolVersion {
        &self.protocol_version
    }
}

pub trait ToolCallLedger: Send + Sync {
    fn record_call(
        &self,
        record: &ToolCallRecord,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
}

#[cfg(test)]
mod tests {
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};

    use crate::AiRunId;

    use super::{
        ToolCallOutcome, ToolCallRecord, ToolCallRecordInput, ToolLedgerError, ToolName,
        ToolPayloadJson, ToolProtocolVersion, ToolRequestId,
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
    fn tool_name_and_request_id_validate_bounds() {
        assert_eq!(ToolName::try_new(""), Err(ToolLedgerError::ToolNameInvalid));
        assert_eq!(
            ToolName::try_new("bad name"),
            Err(ToolLedgerError::ToolNameInvalid)
        );
        assert_eq!(
            ToolName::try_new(&"a".repeat(65_usize)),
            Err(ToolLedgerError::ToolNameInvalid)
        );
        let name = ToolName::try_new("get_portfolio").expect("도구 이름");
        assert_eq!(name.as_str(), "get_portfolio");

        assert_eq!(
            ToolRequestId::try_new(""),
            Err(ToolLedgerError::RequestIdMalformed)
        );
        assert_eq!(
            ToolRequestId::try_new("bad\nid"),
            Err(ToolLedgerError::RequestIdMalformed)
        );
        assert_eq!(
            ToolRequestId::try_new(&"1".repeat(129_usize)),
            Err(ToolLedgerError::RequestIdMalformed)
        );
        assert_eq!(
            ToolRequestId::try_new("req-1").expect("요청 ID").as_str(),
            "req-1"
        );
    }

    #[test]
    fn tool_payload_protocol_and_record_preserve_fields() {
        assert_eq!(
            ToolPayloadJson::try_new(""),
            Err(ToolLedgerError::PayloadBounds)
        );
        assert_eq!(
            ToolPayloadJson::try_new("{\"a\":\"\0\"}"),
            Err(ToolLedgerError::PayloadBounds)
        );
        assert_eq!(
            ToolPayloadJson::try_new(&"a".repeat(65_537_usize)),
            Err(ToolLedgerError::PayloadBounds)
        );
        assert_eq!(
            ToolProtocolVersion::try_new(""),
            Err(ToolLedgerError::ProtocolVersionSyntax)
        );
        assert_eq!(
            ToolProtocolVersion::try_new("2026 07"),
            Err(ToolLedgerError::ProtocolVersionSyntax)
        );
        assert_eq!(
            ToolProtocolVersion::try_new(&"v".repeat(33_usize)),
            Err(ToolLedgerError::ProtocolVersionSyntax)
        );
        let name = ToolName::try_new("get_portfolio").expect("도구 이름");
        let req_id = ToolRequestId::try_new("req-1").expect("요청 ID");
        let args = ToolPayloadJson::try_new("{}").expect("인자");
        let res = ToolPayloadJson::try_new("{\"ok\":true}").expect("결과");
        let proto = ToolProtocolVersion::try_new("2026-07-28").expect("버전");
        let record = ToolCallRecord::new(ToolCallRecordInput {
            run_id: AiRunId::try_new("run-1").expect("run"),
            request_id: req_id.clone(),
            tool_name: name.clone(),
            arguments_json: args.clone(),
            result_json: res.clone(),
            outcome: ToolCallOutcome::Succeeded,
            started_at: ts(100_i64),
            finished_at: ts(101_i64),
            protocol_version: proto.clone(),
        });
        assert_eq!(record.tool_name(), &name);
        assert_eq!(record.request_id(), &req_id);
        assert_eq!(record.arguments_json(), &args);
        assert_eq!(record.result_json(), &res);
        assert_eq!(record.outcome(), ToolCallOutcome::Succeeded);
        assert_eq!(record.started_at(), ts(100_i64));
        assert_eq!(record.finished_at(), ts(101_i64));
        assert_eq!(record.protocol_version(), &proto);
    }
}
