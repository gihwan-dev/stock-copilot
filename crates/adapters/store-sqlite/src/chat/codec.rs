use crate::estimates::codec::{Decoder, Encoder, invalid, put_timestamp, take_timestamp};
use application::{
    AiRunId, ChatCard, ChatCardId, ChatCardKind, ChatFailureReason, ChatRunLedgerEntry,
    ChatSequence, ChatSource, ChatSourceKind, ChatTurnId, ChatTurnStatus, CredentialSource,
    IsolationFailure, ModelName, ModelSelection, ReasoningEffort, ResetTime, ResponseMismatch,
    RetryDelay, RunKind, RunPurpose, StoredAssistantAttachments, StoredAssistantReply,
    StoredChatContent, StoredChatEntry, StoredUserMessage, SystemFailure, TokenUsage,
    UserMessageId,
};
use domain::{Nanosecond, NormalizedUrl, SpanSeconds, TimeSpan, UntrustedText};

type Table<T> = [(T, &'static str)];

fn tag<T: PartialEq + Copy>(table: &Table<T>, value: T) -> rusqlite::Result<&'static str> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map(|(_, name)| *name)
        .ok_or_else(|| invalid(()))
}

fn untag<T: Copy>(table: &Table<T>, raw: &str) -> rusqlite::Result<T> {
    table
        .iter()
        .find(|(_, name)| *name == raw)
        .map(|(value, _)| *value)
        .ok_or_else(|| invalid(()))
}

const ISOLATION: [(IsolationFailure, &str); 10] = [
    (IsolationFailure::Tools, "tools"),
    (IsolationFailure::McpServers, "mcp_servers"),
    (IsolationFailure::Plugins, "plugins"),
    (IsolationFailure::Skills, "skills"),
    (IsolationFailure::SlashCommands, "slash_commands"),
    (IsolationFailure::PermissionMode, "permission_mode"),
    (IsolationFailure::ApiKeySource, "api_key_source"),
    (IsolationFailure::CliVersion, "cli_version"),
    (IsolationFailure::LoginRequired, "login_required"),
    (IsolationFailure::NotInstalled, "not_installed"),
];

const MISMATCH: [(ResponseMismatch, &str); 2] = [
    (ResponseMismatch::Schema, "schema"),
    (ResponseMismatch::Validation, "validation"),
];

const CREDENTIAL: [(CredentialSource, &str); 2] = [
    (CredentialSource::Dart, "dart"),
    (CredentialSource::Fred, "fred"),
];

const SOURCE_KIND: [(ChatSourceKind, &str); 4] = [
    (ChatSourceKind::Tool, "tool"),
    (ChatSourceKind::Disclosure, "disclosure"),
    (ChatSourceKind::News, "news"),
    (ChatSourceKind::Web, "web"),
];

const CARD_KIND: [(ChatCardKind, &str); 6] = [
    (ChatCardKind::Signal, "signal"),
    (ChatCardKind::Proposal, "proposal"),
    (ChatCardKind::Holding, "holding"),
    (ChatCardKind::Quote, "quote"),
    (ChatCardKind::Briefing, "briefing"),
    (ChatCardKind::Disclosure, "disclosure"),
];

const RUN_KIND: [(RunKind, &str); 2] = [
    (RunKind::Collection, "collection"),
    (RunKind::Analysis, "analysis"),
];

const PURPOSE: [(RunPurpose, &str); 15] = [
    (RunPurpose::Chat, "chat"),
    (RunPurpose::MorningBriefing, "morning_briefing"),
    (RunPurpose::IntradayMonitoring, "intraday_monitoring"),
    (RunPurpose::ClosingReview, "closing_review"),
    (RunPurpose::UsMarketCheck, "us_market_check"),
    (RunPurpose::Estimate, "estimate"),
    (RunPurpose::DeepResearch, "deep_research"),
    (RunPurpose::CandidateDiscovery, "candidate_discovery"),
    (RunPurpose::WeeklyReview, "weekly_review"),
    (RunPurpose::NewsClassification, "news_classification"),
    (RunPurpose::NewsSummary, "news_summary"),
    (RunPurpose::WebCollection, "web_collection"),
    (RunPurpose::ThesisReview, "thesis_review"),
    (RunPurpose::ClaimVerification, "claim_verification"),
    (RunPurpose::PairedComparison, "paired_comparison"),
];

const EFFORT: [(ReasoningEffort, &str); 6] = [
    (ReasoningEffort::Default, "default"),
    (ReasoningEffort::Low, "low"),
    (ReasoningEffort::Medium, "medium"),
    (ReasoningEffort::High, "high"),
    (ReasoningEffort::XHigh, "xhigh"),
    (ReasoningEffort::Max, "max"),
];

fn put_span(out: &mut Encoder, span: TimeSpan) {
    let (seconds, nanos) = span.as_parts();
    out.push_u64(*seconds.as_u64());
    out.push_u32(*nanos.as_u32());
}

fn take_span(input: &mut Decoder<'_>) -> rusqlite::Result<TimeSpan> {
    let seconds = SpanSeconds::new(input.next_u64()?);
    let nanos = Nanosecond::new(input.next_u32()?);
    TimeSpan::try_new(seconds, nanos).map_err(invalid)
}

fn put_optional<T>(out: &mut Encoder, value: Option<T>, put: impl FnOnce(&mut Encoder, T)) {
    match value {
        Some(inner) => {
            out.push("some");
            put(out, inner);
        }
        None => out.push("none"),
    }
}

fn take_optional<T>(
    input: &mut Decoder<'_>,
    take: impl FnOnce(&mut Decoder<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Option<T>> {
    match input.next()? {
        "some" => take(input).map(Some),
        "none" => Ok(None),
        _ => Err(invalid(())),
    }
}

fn put_retry(out: &mut Encoder, retry: RetryDelay) {
    match retry {
        RetryDelay::Unknown => out.push("unknown"),
        RetryDelay::Known(span) => {
            out.push("known");
            put_span(out, span);
        }
    }
}

fn take_retry(input: &mut Decoder<'_>) -> rusqlite::Result<RetryDelay> {
    match input.next()? {
        "unknown" => Ok(RetryDelay::Unknown),
        "known" => Ok(RetryDelay::Known(take_span(input)?)),
        _ => Err(invalid(())),
    }
}

fn put_system_failure(out: &mut Encoder, failure: SystemFailure) {
    match failure {
        SystemFailure::PermissionDenied => out.push("permission_denied"),
        SystemFailure::Unavailable(retry) => {
            out.push("unavailable");
            put_retry(out, retry);
        }
        SystemFailure::InvalidData => out.push("invalid_data"),
        SystemFailure::RepositoryIntegrity => out.push("repository_integrity"),
        SystemFailure::SecretIntegrity => out.push("secret_integrity"),
    }
}

fn take_system_failure(input: &mut Decoder<'_>) -> rusqlite::Result<SystemFailure> {
    match input.next()? {
        "permission_denied" => Ok(SystemFailure::PermissionDenied),
        "unavailable" => Ok(SystemFailure::Unavailable(take_retry(input)?)),
        "invalid_data" => Ok(SystemFailure::InvalidData),
        "repository_integrity" => Ok(SystemFailure::RepositoryIntegrity),
        "secret_integrity" => Ok(SystemFailure::SecretIntegrity),
        _ => Err(invalid(())),
    }
}

fn put_failure(out: &mut Encoder, reason: ChatFailureReason) -> rusqlite::Result<()> {
    match reason {
        ChatFailureReason::IsolationRejected(failure) => {
            out.push("isolation_rejected");
            out.push(tag(&ISOLATION, failure)?);
        }
        ChatFailureReason::IsolationNotVerified => out.push("isolation_not_verified"),
        ChatFailureReason::McpUnavailable => out.push("mcp_unavailable"),
        ChatFailureReason::ProviderUnavailable => out.push("provider_unavailable"),
        ChatFailureReason::InvalidResponse(mismatch) => {
            out.push("invalid_response");
            out.push(tag(&MISMATCH, mismatch)?);
        }
        ChatFailureReason::InvalidCredential(source) => {
            out.push("invalid_credential");
            out.push(tag(&CREDENTIAL, source)?);
        }
        ChatFailureReason::ForbiddenToolAttempt => out.push("forbidden_tool_attempt"),
        ChatFailureReason::EmptyMessage => out.push("empty_message"),
        ChatFailureReason::PromptTooLong => out.push("prompt_too_long"),
        ChatFailureReason::StorageError(failure) => {
            out.push("storage_error");
            put_system_failure(out, failure);
        }
        ChatFailureReason::QueueBusy => out.push("queue_busy"),
    }
    Ok(())
}

fn take_failure(input: &mut Decoder<'_>) -> rusqlite::Result<ChatFailureReason> {
    Ok(match input.next()? {
        "isolation_rejected" => {
            ChatFailureReason::IsolationRejected(untag(&ISOLATION, input.next()?)?)
        }
        "isolation_not_verified" => ChatFailureReason::IsolationNotVerified,
        "mcp_unavailable" => ChatFailureReason::McpUnavailable,
        "provider_unavailable" => ChatFailureReason::ProviderUnavailable,
        "invalid_response" => ChatFailureReason::InvalidResponse(untag(&MISMATCH, input.next()?)?),
        "invalid_credential" => {
            ChatFailureReason::InvalidCredential(untag(&CREDENTIAL, input.next()?)?)
        }
        "forbidden_tool_attempt" => ChatFailureReason::ForbiddenToolAttempt,
        "empty_message" => ChatFailureReason::EmptyMessage,
        "prompt_too_long" => ChatFailureReason::PromptTooLong,
        "storage_error" => ChatFailureReason::StorageError(take_system_failure(input)?),
        "queue_busy" => ChatFailureReason::QueueBusy,
        _ => return Err(invalid(())),
    })
}

fn put_status(out: &mut Encoder, status: ChatTurnStatus) -> rusqlite::Result<()> {
    match status {
        ChatTurnStatus::InProgress => out.push("in_progress"),
        ChatTurnStatus::Completed => out.push("completed"),
        ChatTurnStatus::Cancelled => out.push("cancelled"),
        ChatTurnStatus::Interrupted => out.push("interrupted"),
        ChatTurnStatus::RateLimited(reset) => {
            out.push("rate_limited");
            match reset {
                ResetTime::Known(at) => {
                    out.push("known");
                    put_timestamp(out, at);
                }
                ResetTime::NotProvided => out.push("not_provided"),
            }
        }
        ChatTurnStatus::Failed(reason) => {
            out.push("failed");
            put_failure(out, reason)?;
        }
    }
    Ok(())
}

fn take_status(input: &mut Decoder<'_>) -> rusqlite::Result<ChatTurnStatus> {
    Ok(match input.next()? {
        "in_progress" => ChatTurnStatus::InProgress,
        "completed" => ChatTurnStatus::Completed,
        "cancelled" => ChatTurnStatus::Cancelled,
        "interrupted" => ChatTurnStatus::Interrupted,
        "rate_limited" => ChatTurnStatus::RateLimited(match input.next()? {
            "known" => ResetTime::Known(take_timestamp(input)?),
            "not_provided" => ResetTime::NotProvided,
            _ => return Err(invalid(())),
        }),
        "failed" => ChatTurnStatus::Failed(take_failure(input)?),
        _ => return Err(invalid(())),
    })
}

fn put_model(out: &mut Encoder, model: &ModelName) {
    out.push(model.as_str());
}

fn take_model(input: &mut Decoder<'_>) -> rusqlite::Result<ModelName> {
    ModelName::try_new(input.next()?).map_err(invalid)
}

fn take_text(input: &mut Decoder<'_>) -> rusqlite::Result<UntrustedText> {
    UntrustedText::try_new(input.next()?).map_err(invalid)
}

fn put_source(out: &mut Encoder, source: &ChatSource) -> rusqlite::Result<()> {
    out.push(tag(&SOURCE_KIND, source.kind())?);
    out.push(source.reference().as_untrusted_str());
    put_optional(out, source.url(), |target, url| target.push(url.as_str()));
    Ok(())
}

fn take_source(input: &mut Decoder<'_>) -> rusqlite::Result<ChatSource> {
    let kind = untag(&SOURCE_KIND, input.next()?)?;
    let reference = take_text(input)?;
    let url = take_optional(input, |source| {
        NormalizedUrl::try_new(source.next()?).map_err(invalid)
    })?;
    Ok(ChatSource::new(kind, reference, url))
}

fn put_card(out: &mut Encoder, card: &ChatCard) -> rusqlite::Result<()> {
    out.push(tag(&CARD_KIND, card.kind())?);
    out.push(card.id().as_str());
    Ok(())
}

fn take_card(input: &mut Decoder<'_>) -> rusqlite::Result<ChatCard> {
    let kind = untag(&CARD_KIND, input.next()?)?;
    let id = ChatCardId::try_new(input.next()?).map_err(invalid)?;
    Ok(ChatCard::new(kind, id))
}

fn put_assistant(out: &mut Encoder, reply: &StoredAssistantReply) -> rusqlite::Result<()> {
    out.push(reply.run_id().as_str());
    out.push(reply.text().as_untrusted_str());
    put_status(out, reply.status())?;
    put_optional(out, reply.actual_model(), put_model);
    out.push_usize(reply.sources().len());
    for source in reply.sources() {
        put_source(out, source)?;
    }
    out.push_usize(reply.cards().len());
    for card in reply.cards() {
        put_card(out, card)?;
    }
    Ok(())
}

fn take_assistant(input: &mut Decoder<'_>) -> rusqlite::Result<StoredAssistantReply> {
    let run_id = AiRunId::try_new(input.next()?).map_err(invalid)?;
    let text = take_text(input)?;
    let status = take_status(input)?;
    let actual_model = take_optional(input, take_model)?;
    let source_count = input.next_usize()?;
    let mut sources = Vec::new();
    for _ in 0..source_count {
        sources.push(take_source(input)?);
    }
    let card_count = input.next_usize()?;
    let mut cards = Vec::new();
    for _ in 0..card_count {
        cards.push(take_card(input)?);
    }
    Ok(StoredAssistantReply::new(
        run_id,
        text,
        status,
        StoredAssistantAttachments {
            actual_model,
            sources,
            cards,
        },
    ))
}

pub(crate) fn encode_entry(entry: &StoredChatEntry) -> rusqlite::Result<String> {
    let mut out = Encoder::new();
    out.push_u64(*entry.sequence().as_u64());
    put_timestamp(&mut out, entry.at());
    out.push(entry.turn_id().as_str());
    match entry.content() {
        StoredChatContent::User(message) => {
            out.push("user");
            out.push(message.id().as_str());
            out.push(message.text().as_untrusted_str());
        }
        StoredChatContent::Assistant(reply) => {
            out.push("assistant");
            put_assistant(&mut out, reply)?;
        }
    }
    Ok(out.finish())
}

pub(crate) fn decode_entry(payload: &str) -> rusqlite::Result<StoredChatEntry> {
    let mut input = Decoder::new(payload);
    let sequence = ChatSequence::new(input.next_u64()?);
    let at = take_timestamp(&mut input)?;
    let turn = ChatTurnId::try_new(input.next()?).map_err(invalid)?;
    let content = match input.next()? {
        "user" => {
            let id = UserMessageId::try_new(input.next()?).map_err(invalid)?;
            let text = take_text(&mut input)?;
            StoredChatContent::User(StoredUserMessage::from_store_result(id, text))
        }
        "assistant" => StoredChatContent::Assistant(take_assistant(&mut input)?),
        _ => return Err(invalid(())),
    };
    input.finish()?;
    Ok(StoredChatEntry::new(sequence, at, turn, content))
}

pub(crate) fn user_message_id(entry: &StoredChatEntry) -> Option<&str> {
    match entry.content() {
        StoredChatContent::User(message) => Some(message.id().as_str()),
        StoredChatContent::Assistant(_) => None,
    }
}

fn put_usage(out: &mut Encoder, usage: &TokenUsage) {
    for count in [
        usage.input(),
        usage.output(),
        usage.cache_read(),
        usage.cache_creation(),
    ] {
        out.push_u64(*count.as_u64());
    }
}

fn put_selection(out: &mut Encoder, selection: &ModelSelection) {
    match selection {
        ModelSelection::Default => out.push("default"),
        ModelSelection::Highest => out.push("highest"),
        ModelSelection::Fast => out.push("fast"),
        ModelSelection::Named(name) => {
            out.push("named");
            put_model(out, name);
        }
    }
}

pub(crate) fn encode_run(entry: &ChatRunLedgerEntry) -> rusqlite::Result<String> {
    let mut out = Encoder::new();
    out.push(entry.run_id().as_str());
    out.push(entry.session_id().as_str());
    out.push(entry.turn_id().as_str());
    out.push(tag(&RUN_KIND, entry.kind())?);
    out.push(tag(&PURPOSE, entry.purpose())?);
    put_selection(&mut out, entry.requested_model());
    put_optional(&mut out, entry.actual_model(), put_model);
    out.push(tag(&EFFORT, entry.effort())?);
    put_optional(&mut out, entry.usage(), put_usage);
    put_timestamp(&mut out, entry.started_at());
    put_timestamp(&mut out, entry.finished_at());
    put_status(&mut out, entry.status())?;
    put_optional(&mut out, entry.first_token_latency(), put_span);
    Ok(out.finish())
}
