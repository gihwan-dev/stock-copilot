use crate::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AnalysisPrompt, Clock, CollectionQuery,
    CollectionTopic, DeliveryReport, EventBus, IsolationCheck, ModelSelection, ReasoningEffort,
    RunInput, RunPurpose, RunSpec, SystemPortError, WriteCompleted,
};
use domain::{
    BriefingId, BriefingKind, DomainEvent, InformationSource, NormalizedUrl, OffsetSeconds, Symbol,
    Timestamp, UntrustedText,
};
use std::fmt::Write as _;
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefingSource {
    pub title: UntrustedText,
    pub url: Option<NormalizedUrl>,
    pub origin: InformationSource,
    pub observed_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BriefingFailure {
    ProviderUnavailable,
    IsolationRejected,
    RateLimited,
    EmptyResult,
    PromptRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BriefingOutcome {
    Completed {
        body: UntrustedText,
        sources: Vec<BriefingSource>,
    },
    Failed(BriefingFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefingRecord {
    pub id: BriefingId,
    pub finished_at: Timestamp,
    pub outcome: BriefingOutcome,
}

pub trait BriefingRepository: Send + Sync {
    fn save(
        &self,
        record: &BriefingRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn find(
        &self,
        id: &BriefingId,
    ) -> impl Future<Output = Result<Option<BriefingRecord>, SystemPortError>> + Send;
    fn recent(&self) -> impl Future<Output = Result<Vec<BriefingRecord>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BriefingContext {
    pub instruments: Vec<Symbol>,
    pub evidence: Vec<BriefingSource>,
    pub code_facts: Vec<UntrustedText>,
}

pub trait BriefingContextSource: Send + Sync {
    fn context(
        &self,
        kind: BriefingKind,
        now: Timestamp,
    ) -> impl Future<Output = Result<BriefingContext, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BriefingRunError {
    Time,
    Context(SystemPortError),
    Storage(SystemPortError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefingRunReport {
    pub record: BriefingRecord,
    pub delivery: Option<DeliveryReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectionStep {
    Collect,
    AnalysisOnly,
}

const fn collection_step(kind: BriefingKind) -> CollectionStep {
    match kind {
        BriefingKind::Morning | BriefingKind::PreUsMarket => CollectionStep::Collect,
        BriefingKind::ClosingReview => CollectionStep::AnalysisOnly,
    }
}

const fn purpose(kind: BriefingKind) -> RunPurpose {
    match kind {
        BriefingKind::Morning => RunPurpose::MorningBriefing,
        BriefingKind::ClosingReview => RunPurpose::ClosingReview,
        BriefingKind::PreUsMarket => RunPurpose::UsMarketCheck,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunPhase {
    Collect,
    Analysis,
}

fn run_id(kind: BriefingKind, now: Timestamp, run_phase: RunPhase) -> Option<AiRunId> {
    let seconds = *now.utc_seconds().as_i64();
    let phase = match run_phase {
        RunPhase::Collect => "collect",
        RunPhase::Analysis => "analysis",
    };
    let code = match kind {
        BriefingKind::Morning => "morning",
        BriefingKind::ClosingReview => "closing",
        BriefingKind::PreUsMarket => "pre-us",
    };
    AiRunId::try_new(&format!("briefing-{code}-{phase}-{seconds}")).ok()
}

fn build_prompt(kind: BriefingKind, context: &BriefingContext) -> Option<AnalysisPrompt> {
    let mut text = String::from(match kind {
        BriefingKind::Morning => {
            "아침 브리핑을 한국어로 쓴다. 밤사이 미국장, 환율, 보유 종목의 뉴스와 공시, 오늘 일정을 짧게 정리한다. 매수·매도 권유나 수량·가격 제안은 쓰지 않는다. 문장마다 근거가 된 자료 번호를 [n] 형식으로 붙인다. 자료에 없는 사실은 쓰지 않는다."
        }
        BriefingKind::ClosingReview => {
            "마감 리뷰를 한국어로 쓴다. 코드가 계산한 오늘의 변동 분해 수치는 그대로 인용하고 다시 계산하지 않는다. 움직임의 원인 후보만 자료 번호 [n]과 함께 제시하고, 확인되지 않은 원인은 후보라고 밝힌다. 매수·매도 권유는 쓰지 않는다."
        }
        BriefingKind::PreUsMarket => {
            "미국장 개장 전 점검을 한국어로 쓴다. 미국 보유 종목의 뉴스와 실적 일정을 짧게 정리한다. 매수·매도 권유는 쓰지 않는다. 문장마다 근거 자료 번호를 [n] 형식으로 붙인다."
        }
    });
    text.push_str("\n\n<code_facts>\n");
    for fact in &context.code_facts {
        let _written = writeln!(text, "{}", fact.as_untrusted_str());
    }
    text.push_str("</code_facts>\n\n<untrusted_text>\n");
    for (index, source) in context.evidence.iter().enumerate() {
        let _written = writeln!(
            text,
            "[{}] ({}) {}",
            index.saturating_add(1),
            match &source.origin {
                InformationSource::Web => "web",
                InformationSource::News(_) => "news",
                InformationSource::Disclosure(_) => "disclosure",
            },
            source.title.as_untrusted_str()
        );
    }
    text.push_str("</untrusted_text>\n");
    AnalysisPrompt::try_new(&text).ok()
}

#[derive(Debug)]
enum Drained {
    Text(UntrustedText),
    Failed(BriefingFailure),
}

async fn drain<S: AiEventStream>(stream: &mut S) -> Drained {
    let mut text = String::new();
    loop {
        match stream.next().await {
            Ok(Some(event)) => match event {
                AiEvent::TextDelta(delta) => text.push_str(delta.as_untrusted_str()),
                AiEvent::IsolationChecked(IsolationCheck::Rejected(_)) => {
                    return Drained::Failed(BriefingFailure::IsolationRejected);
                }
                AiEvent::RateLimited { .. } => {
                    return Drained::Failed(BriefingFailure::RateLimited);
                }
                AiEvent::IsolationChecked(IsolationCheck::Passed)
                | AiEvent::ToolCall { .. }
                | AiEvent::WebSearch { .. }
                | AiEvent::TokenUsage(_)
                | AiEvent::Quota(_)
                | AiEvent::ModelRerouted(_)
                | AiEvent::ActualModel(_)
                | AiEvent::McpUnavailable
                | AiEvent::Unknown(_) => {}
            },
            Ok(None) => {
                let mut end = text.len().min(65_536);
                while !text.is_char_boundary(end) {
                    end = end.saturating_sub(1);
                }
                let body = text
                    .get(..end)
                    .and_then(|slice| UntrustedText::try_new(slice).ok())
                    .unwrap_or_else(UntrustedText::empty);
                return Drained::Text(body);
            }
            Err(_error) => return Drained::Failed(BriefingFailure::ProviderUnavailable),
        }
    }
}

#[derive(Debug)]
pub struct RunBriefing<'a, Ai, Repository, Context, Time> {
    pub ai: &'a Ai,
    pub repository: &'a Repository,
    pub context: &'a Context,
    pub clock: &'a Time,
    pub events: &'a EventBus,
}

impl<Ai, Repository, Context, Time> RunBriefing<'_, Ai, Repository, Context, Time>
where
    Ai: AiProvider,
    Repository: BriefingRepository,
    Context: BriefingContextSource,
    Time: Clock,
{
    pub async fn execute(&self, kind: BriefingKind) -> Result<BriefingRunReport, BriefingRunError> {
        let started_at = self.clock.now();
        let date = started_at
            .date_at_offset(OffsetSeconds::new(32_400))
            .map_err(|_error| BriefingRunError::Time)?;
        let id = BriefingId::new(kind, date);
        let mut context = self
            .context
            .context(kind, started_at)
            .await
            .map_err(BriefingRunError::Context)?;
        if collection_step(kind) == CollectionStep::Collect
            && !context.instruments.is_empty()
            && let Some(source) = self.collect(kind, started_at, &context).await
        {
            context.evidence.push(source);
        }
        let outcome = self.analyze(kind, started_at, &context).await;
        let record = BriefingRecord {
            id,
            finished_at: self.clock.now(),
            outcome,
        };
        self.repository
            .save(&record)
            .await
            .map_err(BriefingRunError::Storage)?;
        let delivery = match record.outcome {
            BriefingOutcome::Completed { .. } => {
                Some(self.events.publish(DomainEvent::BriefingFinished(id)))
            }
            BriefingOutcome::Failed(_) => None,
        };
        Ok(BriefingRunReport { record, delivery })
    }

    async fn collect(
        &self,
        kind: BriefingKind,
        now: Timestamp,
        context: &BriefingContext,
    ) -> Option<BriefingSource> {
        let spec = RunSpec::new(
            run_id(kind, now, RunPhase::Collect)?,
            RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: context.instruments.clone(),
                topics: vec![
                    CollectionTopic::CompanyNews,
                    CollectionTopic::Disclosures,
                    CollectionTopic::Earnings,
                ],
            }),
            ModelSelection::Fast,
            ReasoningEffort::Low,
            RunPurpose::WebCollection,
        );
        let mut stream = self.ai.run(spec).await.ok()?;
        match drain(&mut stream).await {
            Drained::Text(text) if !text.as_untrusted_str().trim().is_empty() => {
                Some(BriefingSource {
                    title: text,
                    url: None,
                    origin: InformationSource::Web,
                    observed_at: self.clock.now(),
                })
            }
            Drained::Text(_) | Drained::Failed(_) => None,
        }
    }

    async fn analyze(
        &self,
        kind: BriefingKind,
        now: Timestamp,
        context: &BriefingContext,
    ) -> BriefingOutcome {
        let Some(prompt) = build_prompt(kind, context) else {
            return BriefingOutcome::Failed(BriefingFailure::PromptRejected);
        };
        let Some(id) = run_id(kind, now, RunPhase::Analysis) else {
            return BriefingOutcome::Failed(BriefingFailure::PromptRejected);
        };
        let spec = RunSpec::new(
            id,
            RunInput::Analysis(prompt),
            ModelSelection::Default,
            ReasoningEffort::Default,
            purpose(kind),
        );
        let mut stream = match self.ai.run(spec).await {
            Ok(stream) => stream,
            Err(_error) => return BriefingOutcome::Failed(BriefingFailure::ProviderUnavailable),
        };
        match drain(&mut stream).await {
            Drained::Text(text) if text.as_untrusted_str().trim().is_empty() => {
                BriefingOutcome::Failed(BriefingFailure::EmptyResult)
            }
            Drained::Text(text) => BriefingOutcome::Completed {
                body: text,
                sources: context.evidence.clone(),
            },
            Drained::Failed(failure) => BriefingOutcome::Failed(failure),
        }
    }
}
