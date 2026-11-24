use std::sync::Arc;

use application::OrderConfirmation;
use approval_command::execute::ApprovalCommandOutcome;
use approval_command::unknown::UnknownRetryOutcome;
use domain::{BrokerOrderId, ClientOrderId, Nanosecond, ProposalId, SpanSeconds, TimeSpan};
use testkit_ports::ScriptedPresence;

use super::fixture::{Options, PROPOSAL, PathFixture, ReplyKind, fixture_with};

fn unknown_fixture(reply: ReplyKind) -> PathFixture {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    fixture_with(
        presence,
        "BRK-O19",
        Options {
            reply,
            ..Options::default()
        },
    )
}

fn key() -> ClientOrderId {
    ClientOrderId::try_new(PROPOSAL).expect("주문 키")
}

fn submit_unknown(path: &PathFixture) {
    let id = ProposalId::new(key());
    let outcome = tauri::async_runtime::block_on(path.service.request(&id)).expect("승인 실행");
    match outcome {
        ApprovalCommandOutcome::OutcomeUnknown { .. } => {}
        other @ (ApprovalCommandOutcome::Rejected(_)
        | ApprovalCommandOutcome::Submitted { .. }
        | ApprovalCommandOutcome::BrokerRejected { .. }) => {
            panic!("결과 모름이어야 함: {other:?}")
        }
    }
}

fn advance(path: &PathFixture, seconds: u64) {
    let span = TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("간격");
    let _now = path.clock.advance_by(span).expect("시계");
}

#[test]
fn retry_within_ten_minutes_resends_the_same_key_and_content_once() {
    let path = unknown_fixture(ReplyKind::TransportThenAccept);
    submit_unknown(&path);
    advance(&path, 599);
    let retried = tauri::async_runtime::block_on(path.service.retry_unknown(&key()));
    match retried {
        UnknownRetryOutcome::Resolved {
            broker_order_id, ..
        } => assert_eq!(
            broker_order_id,
            BrokerOrderId::try_new("BRK-O19").expect("주문번호")
        ),
        other @ (UnknownRetryOutcome::NotRetryable
        | UnknownRetryOutcome::WindowExpired
        | UnknownRetryOutcome::ClockRollback
        | UnknownRetryOutcome::Rejected { .. }
        | UnknownRetryOutcome::StillUnknown { .. }) => panic!("확정이어야 함: {other:?}"),
    }
    let sent = path.trade.sent();
    assert_eq!(sent.len(), 2, "처음 1건과 재요청 1건");
    assert_eq!(sent[0].client_order_id, sent[1].client_order_id, "같은 키");
    assert_eq!(sent[0].intent, sent[1].intent, "같은 내용");
    assert_eq!(sent[0].summary, sent[1].summary, "같은 요약");
    assert_eq!(
        tauri::async_runtime::block_on(path.service.retry_unknown(&key())),
        UnknownRetryOutcome::NotRetryable,
        "확정 뒤 재요청 0건"
    );
    assert_eq!(path.trade.sent().len(), 2);
}

#[test]
fn retry_after_ten_minutes_sends_nothing() {
    let path = unknown_fixture(ReplyKind::TransportThenAccept);
    submit_unknown(&path);
    advance(&path, 600);
    assert_eq!(
        tauri::async_runtime::block_on(path.service.retry_unknown(&key())),
        UnknownRetryOutcome::WindowExpired
    );
    assert_eq!(path.trade.sent().len(), 1, "10분 뒤 재전송 0건");
    assert_eq!(
        tauri::async_runtime::block_on(path.service.retry_unknown(&key())),
        UnknownRetryOutcome::NotRetryable,
        "만료된 허가는 다시 쓰지 않음"
    );
}

#[test]
fn maintenance_unknown_is_never_resent() {
    let path = unknown_fixture(ReplyKind::Maintenance);
    submit_unknown(&path);
    assert_eq!(
        tauri::async_runtime::block_on(path.service.retry_unknown(&key())),
        UnknownRetryOutcome::NotRetryable
    );
    assert_eq!(path.trade.sent().len(), 1, "maintenance 뒤 재전송 0건");
}
