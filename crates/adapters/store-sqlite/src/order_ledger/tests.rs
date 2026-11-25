use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, SharedDatabase, SqliteRepositories};
use application::{
    AuditIntegrity, AuditLog, ConsumptionOutcome, ConsumptionStatus, OrderRateRepository,
    ProposalConsumptionRepository,
};
use domain::{ClientOrderId, ProposalId};

fn proposal() -> (ProposalId, ClientOrderId) {
    let id = ClientOrderId::try_new("sc-ord-0001").expect("주문 번호");
    (ProposalId::new(id.clone()), id)
}

#[test]
fn prepared_submission_consumes_counts_and_audits_once_in_one_transaction() {
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    let (proposal, client) = proposal();
    runtime().block_on(async {
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        let at = timestamp();
        assert_eq!(
            crate::commit_prepared_submission(&repositories, &proposal, &client, at).await,
            Ok(ConsumptionOutcome::Recorded)
        );
        assert_eq!(
            crate::commit_prepared_submission(&repositories, &proposal, &client, at).await,
            Ok(ConsumptionOutcome::AlreadyConsumed),
            "같은 제안은 두 번 보내지 않는다"
        );
        assert_eq!(
            ProposalConsumptionRepository::status(&repositories, &proposal).await,
            Ok(ConsumptionStatus::Consumed)
        );
        let times = repositories
            .recent_submission_times(at)
            .await
            .expect("주문 시각");
        assert_eq!(times, vec![at], "시간당 주문 수에 한 번만 센다");
        assert_eq!(repositories.verify().await, Ok(AuditIntegrity::Verified));
    });
}

#[test]
fn without_a_database_nothing_is_consumed() {
    let repositories = SqliteRepositories::new(SharedDatabase::new());
    let (proposal, client) = proposal();
    let result = runtime().block_on(crate::commit_prepared_submission(
        &repositories,
        &proposal,
        &client,
        timestamp(),
    ));
    let _error = result.expect_err("DB 없음");
}

#[test]
fn unknown_proposal_has_no_instrument() {
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    let (proposal, _client) = proposal();
    runtime().block_on(async {
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        assert_eq!(
            crate::proposal_instrument(&repositories, &proposal).await,
            Ok(None)
        );
    });
}
