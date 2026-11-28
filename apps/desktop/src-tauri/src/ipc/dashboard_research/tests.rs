use super::{
    Cached, DashboardCacheKey, ResearchSection, ResearchStatus, current_section, inputs, section,
};
use crate::tests::support::{TestPorts, ports};
use application::Clock;
use domain::{
    AccountId, BuyingPower, Currency, Money, NumericInput, Portfolio, SyncOutcome, SyncRecord,
    Timestamp,
};
use std::sync::Arc;
use tauri::test::{mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;
use tokio::sync::Mutex;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        domain::UnixSeconds::new(seconds),
        domain::Nanosecond::new(0),
        domain::OffsetSeconds::new(0),
    )
    .expect("합성 시각")
}

#[test]
fn cache_hit_checks_age_clock_direction_and_scope_even_after_serialization() {
    let scope = |share: &str| {
        let source = application::BriefingSource {
            title: domain::UntrustedText::try_new(share).expect("비중"),
            url: None,
            origin: domain::InformationSource::Web,
            observed_at: at(10),
        };
        inputs::valuation_scope("동일 계좌와 종목", &[source]).expect("평가 근거")
    };
    let before = scope("합성 종목 비중 10%");
    let after = scope("합성 종목 비중 90%");
    assert_ne!(before, after);
    let cached = Cached {
        scope: before.clone(),
        at: 10,
        section: ResearchSection::status(ResearchStatus::Ready),
    };
    let stored = serde_json::to_string(&cached).expect("저장");
    assert!(current_section(Some(&stored), &before, at(21_609)).is_some());
    assert!(current_section(Some(&stored), &after, at(11)).is_none());
    assert!(current_section(Some(&stored), &before, at(21_610)).is_none());
    assert!(current_section(Some(&stored), &before, at(9)).is_none());
}

#[test]
fn concurrent_read_returns_refreshing_without_a_second_ai_call() {
    let app = mock_builder()
        .manage(ports())
        .build(mock_context(noop_assets()))
        .expect("시험 앱");
    let handle = app.handle();
    let lock = Arc::new(Mutex::new(None));
    tauri::async_runtime::block_on(async {
        let _running = lock.lock().await;
        let read =
            section::<_, TestPorts>(handle, DashboardCacheKey::Economy, None, Arc::clone(&lock))
                .await;
        assert_eq!(read.status, ResearchStatus::Refreshing);
        let managed = tauri::Manager::state::<crate::ports::Ports<TestPorts>>(handle);
        assert_eq!(managed.ai_provider.calls(), Vec::new());
    });
}

#[test]
fn missing_incomplete_or_inaccessible_inventory_cannot_authorize_discovery() {
    let ports = ports();
    let account = AccountId::new(1);
    let _selected = ports.saved_accounts.replace(vec![account]);
    let at = ports.clock.now();
    let cash = |currency| {
        BuyingPower::new(
            Money::parse(&NumericInput::try_new("0").expect("합성 값"), currency).expect("금액"),
            at,
        )
    };
    let empty =
        Portfolio::new(Vec::new(), cash(Currency::Krw), cash(Currency::Usd)).expect("빈 보유");
    tauri::async_runtime::block_on(async {
        ports
            .portfolios
            .script_latest()
            .push(MemoryReply::Return(Ok(None)));
        assert!(inputs::holdings(&ports).await.is_err());
        for bad in [
            SyncOutcome::Failed,
            SyncOutcome::AccessBlocked,
            SyncOutcome::Partial(empty.clone()),
        ] {
            ports
                .portfolios
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(at, bad)))));
            assert!(inputs::holdings(&ports).await.is_err());
        }
        ports
            .portfolios
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at,
                SyncOutcome::Complete(empty),
            )))));
        assert!(inputs::holdings(&ports).await.expect("조회").is_some());
    });
}

#[test]
fn legacy_entries_gain_a_detail_action_only_while_the_master_directory_contains_them() {
    let master = testkit_ports::estimate_fixtures::sample_master().expect("합성 종목 마스터");
    let symbol = master.row().symbol.as_str();
    let mut section = ResearchSection::status(ResearchStatus::Ready);
    let old = serde_json::json!({"symbol":symbol,"body":"합성 근거 [1]"});
    section
        .entries
        .push(serde_json::from_value(old).expect("구형 캐시"));
    let directory = crate::holdings_link::InstrumentDirectory::default();
    super::refresh_details(&mut section, Some(&directory));
    let first = section.entries.first().expect("후보");
    assert_eq!(first.detail, super::ResearchDetail::Unavailable);
    assert_eq!(first.display_name, None);
    directory.replace(std::slice::from_ref(&master));
    super::refresh_details(&mut section, Some(&directory));
    let found = section.entries.first().expect("후보");
    assert_eq!(found.detail, super::ResearchDetail::Available);
    assert_eq!(
        found.display_name.as_deref(),
        Some(master.row().name.as_str())
    );
    directory.replace(&[]);
    super::refresh_details(&mut section, Some(&directory));
    assert_eq!(
        section.entries.first().expect("후보").detail,
        super::ResearchDetail::Unavailable
    );
}

#[test]
fn sector_changes_survive_an_overview_outage_and_incomplete_reads_are_distinct_from_empty() {
    use application::{
        AllocationLabel, DashboardSubject, MarketOverviewToolView, SectorToolLine, SectorsToolView,
        SystemFailure, SystemPort, SystemPortError,
    };
    use testkit_ports::tool_read_fakes::FixedMarketToolReads;
    let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
    let sectors = SectorsToolView {
        market: domain::KnownMarket::Kr,
        collected_at: at(91),
        lines: vec![SectorToolLine {
            label: AllocationLabel::try_new("합성 업종").expect("업종"),
            change_rate: Some(
                domain::Ratio::parse(&NumericInput::try_new("0.02").expect("숫자"))
                    .expect("변화율"),
            ),
            trading_amount: None,
        }],
    };
    let available = FixedMarketToolReads::default()
        .with_overview(Err(failure))
        .with_sectors(Ok(Some(sectors)));
    let incomplete = FixedMarketToolReads::default()
        .with_overview(Ok(Some(MarketOverviewToolView {
            lines: Vec::new(),
            sessions: Vec::new(),
        })))
        .with_sectors(Err(failure));
    tauri::async_runtime::block_on(async {
        let (subject, evidence) = inputs::economy(&available)
            .await
            .expect("부분 성공")
            .expect("업종 자료");
        assert_eq!(subject, DashboardSubject::Economy);
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence.first().expect("업종").observed_at, at(91));
        inputs::economy(&incomplete)
            .await
            .expect_err("자료 조회 실패");
        assert_eq!(
            inputs::economy(&FixedMarketToolReads::default()).await,
            Ok(None)
        );
    });
}
