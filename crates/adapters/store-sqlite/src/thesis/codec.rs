use domain::{
    ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict, CriterionAssessment,
    CriterionComparison, CriterionKind, CriterionMetric, CriterionObservation, CriterionValue,
    CriterionVerdict, DartReceiptNo, Disclosure, Evidence, EvidenceRef, EvidenceUrl, HistoryAction,
    KillCriterion, KindReceiptNo, MachineCondition, NewsSource, NormalizedUrl, NumericInput,
    ProposedChange, Ratio, ReceiptNo, SecEdgarReceiptNo, ThesisOperation, ThesisVersion,
    ToolReference, UntrustedText, UserDecision, UserReason,
};

fn invalid<E>(_: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
struct Encoder(String);
impl Encoder {
    fn push(&mut self, value: &str) {
        self.0.push_str(&value.len().to_string());
        self.0.push(':');
        self.0.push_str(value);
    }
}
struct Decoder<'a>(&'a str);
impl<'a> Decoder<'a> {
    fn next(&mut self) -> rusqlite::Result<&'a str> {
        let (length, rest) = self.0.split_once(':').ok_or_else(|| invalid(()))?;
        let size = length.parse::<usize>().map_err(invalid)?;
        let value = rest.get(..size).ok_or_else(|| invalid(()))?;
        self.0 = rest.get(size..).ok_or_else(|| invalid(()))?;
        Ok(value)
    }
    fn text(&mut self) -> rusqlite::Result<UntrustedText> {
        UntrustedText::try_new(self.next()?).map_err(invalid)
    }
    fn id(&mut self) -> rusqlite::Result<ClaimId> {
        ClaimId::try_new(self.next()?).map_err(invalid)
    }
    fn url(&mut self) -> rusqlite::Result<EvidenceUrl> {
        EvidenceUrl::try_new(NormalizedUrl::try_new(self.next()?).map_err(invalid)?)
            .map_err(invalid)
    }
    fn metric(&mut self) -> rusqlite::Result<CriterionMetric> {
        CriterionMetric::try_new(self.next()?).map_err(invalid)
    }
    fn value(&mut self) -> rusqlite::Result<CriterionValue> {
        Ok(CriterionValue::new(
            Ratio::parse(&NumericInput::try_new(self.next()?).map_err(invalid)?)
                .map_err(invalid)?
                .as_decimal(),
        ))
    }
    fn tool(&mut self) -> rusqlite::Result<ToolReference> {
        ToolReference::try_new(self.next()?).map_err(invalid)
    }
}
fn source_tag(source: &NewsSource) -> &'static str {
    match source {
        NewsSource::Hankyung => "hankyung",
        NewsSource::Maekyung => "maekyung",
        NewsSource::Yonhap => "yonhap",
        NewsSource::Edaily => "edaily",
        NewsSource::BankOfKorea => "bank_of_korea",
        NewsSource::FederalReserve => "federal_reserve",
        NewsSource::GoogleNews => "google_news",
        NewsSource::YahooFinance => "yahoo_finance",
    }
}
fn news(source: &str) -> rusqlite::Result<NewsSource> {
    match source {
        "hankyung" => Ok(NewsSource::Hankyung),
        "maekyung" => Ok(NewsSource::Maekyung),
        "yonhap" => Ok(NewsSource::Yonhap),
        "edaily" => Ok(NewsSource::Edaily),
        "bank_of_korea" => Ok(NewsSource::BankOfKorea),
        "federal_reserve" => Ok(NewsSource::FederalReserve),
        "google_news" => Ok(NewsSource::GoogleNews),
        "yahoo_finance" => Ok(NewsSource::YahooFinance),
        _ => Err(invalid(())),
    }
}
fn verdict(value: CriterionVerdict) -> &'static str {
    match value {
        CriterionVerdict::Met => "met",
        CriterionVerdict::NotMet => "not_met",
        CriterionVerdict::Unverifiable => "unverifiable",
    }
}
fn decode_verdict(value: &str) -> rusqlite::Result<CriterionVerdict> {
    match value {
        "met" => Ok(CriterionVerdict::Met),
        "not_met" => Ok(CriterionVerdict::NotMet),
        "unverifiable" => Ok(CriterionVerdict::Unverifiable),
        _ => Err(invalid(())),
    }
}
fn encode_evidence(out: &mut Encoder, evidence: &Evidence) {
    out.push(&evidence.as_items().len().to_string());
    for item in evidence.as_items() {
        match item {
            EvidenceRef::Web(url) => {
                out.push("web");
                out.push(url.as_url().as_str());
            }
            EvidenceRef::News(source, url) => {
                out.push("news");
                out.push(source_tag(source));
                out.push(url.as_url().as_str());
            }
            EvidenceRef::Disclosure(key, url) => {
                out.push("disclosure");
                match key.receipt_no() {
                    ReceiptNo::Dart(value) => {
                        out.push("dart");
                        out.push(value.as_str());
                    }
                    ReceiptNo::Kind(value) => {
                        out.push("kind");
                        out.push(value.as_str());
                    }
                    ReceiptNo::SecEdgar(value) => {
                        out.push("sec");
                        out.push(value.as_str());
                    }
                }
                out.push(url.as_url().as_str());
            }
            EvidenceRef::Tool(value) => {
                out.push("tool");
                out.push(value.as_str());
            }
        }
    }
}
fn decode_evidence(input: &mut Decoder<'_>) -> rusqlite::Result<Evidence> {
    let count = input.next()?.parse::<usize>().map_err(invalid)?;
    if !(1..=8).contains(&count) {
        return Err(invalid(()));
    }
    let mut items = Vec::new();
    for _ in 0..count {
        items.push(match input.next()? {
            "web" => EvidenceRef::Web(input.url()?),
            "news" => EvidenceRef::News(news(input.next()?)?, input.url()?),
            "tool" => EvidenceRef::Tool(input.tool()?),
            "disclosure" => {
                let tag = input.next()?;
                let raw = input.next()?;
                let receipt = match tag {
                    "dart" => ReceiptNo::Dart(DartReceiptNo::try_new(raw).map_err(invalid)?),
                    "kind" => ReceiptNo::Kind(KindReceiptNo::try_new(raw).map_err(invalid)?),
                    "sec" => ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new(raw).map_err(invalid)?),
                    _ => return Err(invalid(())),
                };
                let url = input.url()?;
                let disclosure = Disclosure::new(
                    receipt,
                    url.as_url().clone(),
                    UntrustedText::try_new("").map_err(invalid)?,
                    None,
                );
                EvidenceRef::Disclosure(disclosure.key().clone(), url)
            }
            _ => return Err(invalid(())),
        });
    }
    Evidence::try_new(items).map_err(invalid)
}
fn encode_operation(out: &mut Encoder, operation: &ThesisOperation) {
    match operation {
        ThesisOperation::AddClaim(claim) => {
            out.push("add_claim");
            out.push(claim.id().as_str());
            out.push(match claim.role() {
                ClaimRole::Core => "core",
                ClaimRole::Supporting => "supporting",
            });
            for item in [
                claim.statement(),
                claim.metric(),
                claim.expected(),
                claim.observe_via(),
                claim.refutation(),
            ] {
                out.push(item.as_untrusted_str());
            }
        }
        ThesisOperation::Judge(id, value) => {
            out.push("judge");
            out.push(id.as_str());
            out.push(match value {
                ClaimVerdict::Intact => "intact",
                ClaimVerdict::Weakened => "weakened",
                ClaimVerdict::Broken => "broken",
                ClaimVerdict::Unverifiable => "unverifiable",
            });
        }
        ThesisOperation::AddKillCriterion(criterion) => encode_criterion(out, criterion),
        ThesisOperation::JudgeCriterion(id, assessment) => {
            out.push("judge_criterion");
            out.push(id.as_str());
            match assessment {
                CriterionAssessment::Ai(value) => {
                    out.push("ai");
                    out.push(verdict(*value));
                }
                CriterionAssessment::Observed(value) => {
                    out.push("observed");
                    out.push(value.metric().as_str());
                    out.push(&value.value().as_decimal().to_string());
                    out.push(value.source().as_str());
                }
            }
        }
    }
}
fn encode_criterion(out: &mut Encoder, criterion: &KillCriterion) {
    out.push("add_criterion");
    out.push(criterion.id().as_str());
    match criterion.kind() {
        CriterionKind::Narrative(text) => {
            out.push("narrative");
            out.push(text.as_untrusted_str());
        }
        CriterionKind::Machine(condition) => {
            out.push("machine");
            out.push(condition.metric().as_str());
            out.push(match condition.comparison() {
                CriterionComparison::AtLeast => "at_least",
                CriterionComparison::AtMost => "at_most",
                CriterionComparison::Equal => "equal",
            });
            out.push(&condition.threshold().as_decimal().to_string());
        }
    }
}
fn decode_operation(input: &mut Decoder<'_>) -> rusqlite::Result<ThesisOperation> {
    match input.next()? {
        "add_claim" => {
            let id = input.id()?;
            let role = match input.next()? {
                "core" => ClaimRole::Core,
                "supporting" => ClaimRole::Supporting,
                _ => return Err(invalid(())),
            };
            let content = ClaimContent::try_new(
                input.text()?,
                input.text()?,
                input.text()?,
                input.text()?,
                input.text()?,
            )
            .map_err(invalid)?;
            Ok(ThesisOperation::AddClaim(Claim::new(id, role, content)))
        }
        "judge" => {
            let id = input.id()?;
            let value = match input.next()? {
                "intact" => ClaimVerdict::Intact,
                "weakened" => ClaimVerdict::Weakened,
                "broken" => ClaimVerdict::Broken,
                "unverifiable" => ClaimVerdict::Unverifiable,
                _ => return Err(invalid(())),
            };
            Ok(ThesisOperation::Judge(id, value))
        }
        "add_criterion" => decode_criterion(input),
        "judge_criterion" => {
            let id = input.id()?;
            let assessment = match input.next()? {
                "ai" => CriterionAssessment::Ai(decode_verdict(input.next()?)?),
                "observed" => CriterionAssessment::Observed(CriterionObservation::new(
                    input.metric()?,
                    input.value()?,
                    input.tool()?,
                )),
                _ => return Err(invalid(())),
            };
            Ok(ThesisOperation::JudgeCriterion(id, assessment))
        }
        _ => Err(invalid(())),
    }
}
fn decode_criterion(input: &mut Decoder<'_>) -> rusqlite::Result<ThesisOperation> {
    let id = input.id()?;
    let kind = match input.next()? {
        "narrative" => CriterionKind::Narrative(input.text()?),
        "machine" => {
            let metric = input.metric()?;
            let comparison = match input.next()? {
                "at_least" => CriterionComparison::AtLeast,
                "at_most" => CriterionComparison::AtMost,
                "equal" => CriterionComparison::Equal,
                _ => return Err(invalid(())),
            };
            CriterionKind::Machine(MachineCondition::new(metric, comparison, input.value()?))
        }
        _ => return Err(invalid(())),
    };
    Ok(ThesisOperation::AddKillCriterion(
        KillCriterion::new(id, kind).map_err(invalid)?,
    ))
}
pub(super) fn encode(action: &HistoryAction) -> String {
    let mut out = Encoder(String::new());
    match action {
        HistoryAction::Change(change) => {
            out.push("change");
            encode_operation(&mut out, change.operation());
            encode_evidence(&mut out, change.evidence());
            out.push(change.reason().reason().as_untrusted_str());
            out.push(change.reason().model().as_untrusted_str());
            out.push(change.reason().prompt_version().as_untrusted_str());
        }
        HistoryAction::Decision(target, decision, reason) => {
            out.push("decision");
            out.push(&target.as_usize().to_string());
            out.push(match decision {
                UserDecision::Confirm => "confirm",
                UserDecision::Reject => "reject",
            });
            out.push(reason.as_untrusted_text().as_untrusted_str());
        }
    }
    out.0
}
pub(super) fn decode(payload: &str) -> rusqlite::Result<HistoryAction> {
    let mut input = Decoder(payload);
    let action = match input.next()? {
        "change" => {
            let operation = decode_operation(&mut input)?;
            let evidence = decode_evidence(&mut input)?;
            let reason = ChangeReason::try_new(input.text()?, input.text()?, input.text()?)
                .map_err(invalid)?;
            HistoryAction::Change(Box::new(ProposedChange::new(operation, evidence, reason)))
        }
        "decision" => {
            let target = ThesisVersion::try_new(input.next()?.parse::<usize>().map_err(invalid)?)
                .map_err(invalid)?;
            let decision = match input.next()? {
                "confirm" => UserDecision::Confirm,
                "reject" => UserDecision::Reject,
                _ => return Err(invalid(())),
            };
            HistoryAction::Decision(
                target,
                decision,
                UserReason::try_new(input.text()?).map_err(invalid)?,
            )
        }
        _ => return Err(invalid(())),
    };
    if !input.0.is_empty() {
        return Err(invalid(()));
    }
    Ok(action)
}
