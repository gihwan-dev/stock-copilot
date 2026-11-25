use std::fs;
use std::path::Path;
use std::sync::Arc;

use application::{SystemFailure, SystemPort, SystemPortError, ThesisEntryTime};
use domain::{
    ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, CommonShare, Currency,
    DisplayInstrumentName, Evidence, EvidenceRef, EvidenceUrl, InstrumentId, KoreanMasterDetail,
    LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
    MasterSecurityType, Nanosecond, NormalizedUrl, NxtTrading, OffsetSeconds, ProposedChange,
    Symbol, Thesis, ThesisOperation, ThesisPatch, Timestamp, TradingSuspension, UnixSeconds,
    UntrustedText, UserReason,
};
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::{
    GetThesisRequest, ThesisIpcState, ThesisNotes, UiClaimStatus, UiThesisChange, get_thesis,
    to_thesis_view,
};
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::error::IpcErrorKind;
use crate::ipc::state::BoxFuture;

const RECORDED_AT: i64 = 1_795_564_800;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn text(value: &str) -> UntrustedText {
    UntrustedText::try_new(value).expect("문장")
}

fn samsung() -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("시장"),
        symbol: Symbol::try_new("005930").expect("심볼"),
        instrument_id: InstrumentId::try_new("inst-1").expect("ID"),
        name: DisplayInstrumentName::try_new("삼성전자").expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").expect("유형")),
        currency: Currency::Krw,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: at(RECORDED_AT),
    })
    .expect("마스터")
}

fn add_claim(thesis: &mut Thesis, id: &str, statement: &str) {
    let claim = Claim::new(
        ClaimId::try_new(id).expect("id"),
        ClaimRole::Core,
        ClaimContent::try_new(
            text(statement),
            text("metric"),
            text("expected"),
            text("observe"),
            text("refutation"),
        )
        .expect("주장"),
    );
    let evidence = Evidence::try_new(vec![EvidenceRef::Web(
        EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/a").expect("url"))
            .expect("근거"),
    )])
    .expect("근거");
    let reason = ChangeReason::try_new(text("reason"), text("model"), text("v1")).expect("이유");
    thesis
        .apply_batch(
            thesis.version(),
            ThesisPatch::try_new(vec![ProposedChange::new(
                ThesisOperation::AddClaim(claim),
                evidence,
                reason,
            )])
            .expect("변경"),
        )
        .expect("적용");
}

fn thesis() -> Thesis {
    let mut thesis = Thesis::new();
    add_claim(&mut thesis, "claim-1", "메모리 가격이 반등한다");
    let first = thesis.history()[0].sequence();
    thesis
        .confirm_by_user(
            thesis.version(),
            first,
            UserReason::try_new(text("확인")).expect("이유"),
        )
        .expect("확인");
    add_claim(&mut thesis, "claim-2", "파운드리 적자가 줄어든다");
    thesis
}

fn times(thesis: &Thesis) -> Vec<ThesisEntryTime> {
    vec![ThesisEntryTime {
        sequence: thesis.history()[0].sequence(),
        recorded_at: at(RECORDED_AT),
    }]
}

#[test]
fn view_marks_pending_claims_and_lists_history_newest_first_with_recorded_times() {
    let notes = thesis();
    let view = to_thesis_view(&notes, &times(&notes));
    let statuses: Vec<(String, UiClaimStatus)> = view
        .claims
        .iter()
        .map(|claim| (claim.text.clone(), claim.status))
        .collect();
    assert_eq!(
        statuses,
        vec![
            (
                String::from("메모리 가격이 반등한다"),
                UiClaimStatus::Confirmed
            ),
            (
                String::from("파운드리 적자가 줄어든다"),
                UiClaimStatus::NeedsReview
            ),
        ]
    );
    let changes: Vec<UiThesisChange> = view.history.iter().map(|row| row.change).collect();
    assert_eq!(
        changes,
        vec![
            UiThesisChange::ClaimAdded,
            UiThesisChange::UserConfirmed,
            UiThesisChange::ClaimAdded,
        ]
    );
    assert_eq!(view.history[0].at, None);
    assert_eq!(
        view.history[2].at.as_deref(),
        Some("2026-11-25T09:00:00+09:00")
    );
    assert_eq!(
        view.history[2].subject.as_deref(),
        Some("메모리 가격이 반등한다")
    );
}

struct FixedNotes {
    thesis: Option<Thesis>,
    failure: Option<SystemPortError>,
}

impl ThesisNotes for FixedNotes {
    fn thesis(
        &self,
        _instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Option<Thesis>, SystemPortError>> {
        let loaded = match (self.failure, self.thesis.as_ref()) {
            (Some(error), _) => Err(error),
            (None, Some(_)) => Ok(Some(thesis())),
            (None, None) => Ok(None),
        };
        Box::pin(std::future::ready(loaded))
    }

    fn times(
        &self,
        _instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<Vec<ThesisEntryTime>, SystemPortError>> {
        Box::pin(std::future::ready(Ok(self
            .thesis
            .as_ref()
            .map_or_else(Vec::new, times))))
    }
}

fn app_with(notes: FixedNotes) -> tauri::App<tauri::test::MockRuntime> {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _state = tauri::Manager::manage(&app, ThesisIpcState(Arc::new(notes)));
    let directory = InstrumentDirectory::default();
    directory.replace(&[samsung()]);
    let _names = tauri::Manager::manage(&app, directory);
    app
}

fn request(symbol: &str) -> GetThesisRequest {
    GetThesisRequest {
        symbol: String::from(symbol),
    }
}

fn assert_snapshot<T: serde::Serialize>(name: &str, value: &T) {
    let path = Path::new("ipc/responses").join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value).expect("serialize");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(recorded, actual, "{} differs", path.display());
}

#[test]
fn thesis_command_answers_the_recorded_view_and_an_empty_note() {
    let app = app_with(FixedNotes {
        thesis: Some(thesis()),
        failure: None,
    });
    let view = tauri::async_runtime::block_on(get_thesis(app.handle().clone(), request("005930")))
        .expect("논거");
    assert_snapshot("get_thesis", &view);
    let empty = app_with(FixedNotes {
        thesis: None,
        failure: None,
    });
    let blank =
        tauri::async_runtime::block_on(get_thesis(empty.handle().clone(), request("005930")))
            .expect("빈 노트");
    assert_eq!(blank.claims.len(), 0);
    assert_eq!(blank.history.len(), 0);
}

#[test]
fn unknown_symbol_storage_failure_and_bad_requests_map_to_closed_errors() {
    let app = app_with(FixedNotes {
        thesis: None,
        failure: Some(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(application::RetryDelay::Unknown),
        )),
    });
    let failed =
        tauri::async_runtime::block_on(get_thesis(app.handle().clone(), request("005930")))
            .expect_err("저장소");
    assert_ne!(failed.kind, IpcErrorKind::Bug);
    let unknown =
        tauri::async_runtime::block_on(get_thesis(app.handle().clone(), request("123456")))
            .expect_err("모르는 종목");
    assert_eq!(unknown.kind, IpcErrorKind::NoData);
    let invalid = tauri::async_runtime::block_on(get_thesis(app.handle().clone(), request("")))
        .expect_err("빈 심볼");
    assert_eq!(invalid.kind, IpcErrorKind::UserFixable);
    let _refused = serde_json::from_str::<GetThesisRequest>(r#"{"symbol":"005930","all":true}"#)
        .expect_err("모르는 필드");
}
