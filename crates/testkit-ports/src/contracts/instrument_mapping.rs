use application::{InstrumentMappingRepository, MappingStoreError};
use domain::{
    InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus,
    InstrumentSource, IsinCode, KnownMarket, MappingAssignment, Symbol,
};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Barrier},
};

pub trait ContractRuntime: Sync {
    fn block_on<F: Future>(&self, future: F) -> F::Output;
}

fn parse_source(market: KnownMarket, symbol: &str, isin: &str) -> Option<InstrumentSource> {
    let parsed_symbol = Symbol::try_new(symbol);
    assert!(parsed_symbol.is_ok(), "계약 증인 심볼이 유효해야 함");
    let parsed_isin = IsinCode::try_new(isin);
    assert!(parsed_isin.is_ok(), "계약 증인 ISIN이 유효해야 함");
    match (parsed_symbol, parsed_isin) {
        (Ok(sym), Ok(code)) => Some(InstrumentSource::new(market, sym, code)),
        (Ok(_) | Err(_), Ok(_) | Err(_)) => None,
    }
}

fn parse_id(value: &str) -> Option<InstrumentId> {
    let parsed = InstrumentId::try_new(value);
    assert!(parsed.is_ok(), "계약 증인 내부 ID가 유효해야 함");
    parsed.ok()
}

fn mapping_err(error: InstrumentMappingError) -> MappingStoreError {
    MappingStoreError::Mapping(error)
}

async fn assert_missing<P: InstrumentMappingRepository>(
    repo: &P,
    source: &InstrumentSource,
    id: &InstrumentId,
) {
    let missing = Err(mapping_err(InstrumentMappingError::Missing));
    assert_eq!(
        repo.resolve(source).await,
        missing,
        "미등록 원본의 resolve는 Missing이어야 함"
    );
    assert_eq!(
        repo.lookup(source).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "미등록 원본의 lookup은 Missing이어야 함"
    );
    assert_eq!(
        repo.for_id(id).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "미등록 ID의 for_id는 Missing이어야 함"
    );
    assert_eq!(
        repo.retire(source).await,
        missing,
        "미등록 원본의 retire는 Missing이어야 함"
    );
}

async fn assert_record_state<P: InstrumentMappingRepository>(
    repo: &P,
    source: &InstrumentSource,
    id: &InstrumentId,
    status: InstrumentMappingStatus,
) {
    let expected = InstrumentMappingRecord::new(source.clone(), id.clone(), status);
    assert_eq!(
        repo.lookup(source).await,
        Ok(expected.clone()),
        "lookup은 저장된 원본·ID·상태를 보존해야 함"
    );
    assert_eq!(
        repo.for_id(id).await,
        Ok(vec![expected]),
        "for_id는 해당 ID의 단일 이력을 보존해야 함"
    );
}

async fn assert_retired_and_reused_rejection<P: InstrumentMappingRepository>(
    repo: &P,
    original: &InstrumentSource,
    independent: &InstrumentSource,
    third: &InstrumentSource,
    ids: (&InstrumentId, &InstrumentId),
) {
    let (first_id, second_id) = ids;
    assert_eq!(
        repo.retire(original).await,
        Ok(first_id.clone()),
        "폐지는 기존 ID를 반환해야 함"
    );
    assert_eq!(
        repo.retire(original).await,
        Ok(first_id.clone()),
        "반복 폐지는 멱등하게 같은 ID를 반환해야 함"
    );
    assert_eq!(
        repo.resolve(original).await,
        Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지된 원본의 resolve는 Retired여야 함"
    );
    assert_eq!(
        repo.ensure(original, first_id).await,
        Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지된 원본의 같은 후보 재발급은 Retired여야 함"
    );
    assert_eq!(
        repo.ensure(original, second_id).await,
        Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지된 원본의 새 후보 재발급은 Retired여야 함"
    );
    assert_record_state(repo, original, first_id, InstrumentMappingStatus::Retired).await;
    assert_eq!(
        repo.ensure(independent, second_id).await,
        Ok(MappingAssignment::Issued(second_id.clone())),
        "무시된 후보는 다른 원본에서 정상 발급 가능해야 함"
    );
    assert_eq!(
        repo.ensure(third, first_id).await,
        Err(mapping_err(InstrumentMappingError::IdAlreadyUsed)),
        "폐지된 ID는 다른 원본에 재사용될 수 없어야 함"
    );
    assert_eq!(
        repo.resolve(third).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "충돌 실패 뒤 원본은 미등록 상태로 남아야 함"
    );
}

pub async fn lifecycle_roundtrip<P: InstrumentMappingRepository>(repo: &P) {
    let Some(original) = parse_source(KnownMarket::Kr, "005930", "KR7005930003") else {
        return;
    };
    let Some(independent) = parse_source(KnownMarket::Us, "AAPL", "US0378331005") else {
        return;
    };
    let Some(third) = parse_source(KnownMarket::Us, "MSFT", "US5949181045") else {
        return;
    };
    let (Some(first_id), Some(second_id)) = (parse_id("opaque-1"), parse_id("opaque-2")) else {
        return;
    };

    assert_missing(repo, &original, &first_id).await;
    assert_eq!(
        repo.ensure(&original, &first_id).await,
        Ok(MappingAssignment::Issued(first_id.clone())),
        "최초 발급은 Issued를 반환해야 함"
    );
    assert_eq!(
        repo.resolve(&original).await,
        Ok(first_id.clone()),
        "발급 직후 resolve는 기존 ID를 반환해야 함"
    );
    assert_eq!(
        repo.ensure(&original, &second_id).await,
        Ok(MappingAssignment::Existing(first_id.clone())),
        "기존 활성 원본은 후보 ID를 무시하고 Existing을 반환해야 함"
    );
    assert_record_state(repo, &original, &first_id, InstrumentMappingStatus::Active).await;
    assert_retired_and_reused_rejection(
        repo,
        &original,
        &independent,
        &third,
        (&first_id, &second_id),
    )
    .await;
}

async fn assert_all_uncertain<P: InstrumentMappingRepository>(
    repo: &P,
    candidates: &[InstrumentSource],
    unused_id: &InstrumentId,
) {
    for candidate in candidates {
        assert_eq!(
            repo.resolve(candidate).await,
            Err(mapping_err(InstrumentMappingError::IdentityUncertain)),
            "겹치는 원본의 resolve는 IdentityUncertain이어야 함"
        );
        assert_eq!(
            repo.lookup(candidate).await,
            Err(mapping_err(InstrumentMappingError::IdentityUncertain)),
            "겹치는 원본의 lookup은 IdentityUncertain이어야 함"
        );
        assert_eq!(
            repo.ensure(candidate, unused_id).await,
            Err(mapping_err(InstrumentMappingError::IdentityUncertain)),
            "겹치는 원본의 ensure는 IdentityUncertain이어야 함"
        );
        assert_eq!(
            repo.retire(candidate).await,
            Err(mapping_err(InstrumentMappingError::IdentityUncertain)),
            "겹치는 원본의 retire는 IdentityUncertain이어야 함"
        );
    }
}

async fn verify_mixed_case_and_relisting<P: InstrumentMappingRepository>(
    repo: &P,
    original: &InstrumentSource,
    original_id: &InstrumentId,
    overlaps: &[InstrumentSource],
) {
    let Some(mixed_case) = parse_source(KnownMarket::Us, "brk.B", "us0846707026") else {
        return;
    };
    let Some(mixed_variant) = parse_source(KnownMarket::Us, "BRK.B", "us0846707026") else {
        return;
    };
    let Some(unrelated) = parse_source(KnownMarket::Us, "NVDA", "US67066G1040") else {
        return;
    };
    let (Some(brk_id), Some(relist_id)) = (parse_id("opaque-brk"), parse_id("opaque-relist"))
    else {
        return;
    };

    assert_eq!(
        repo.ensure(&mixed_case, &brk_id).await,
        Ok(MappingAssignment::Issued(brk_id.clone())),
        "대소문자 혼합 원본이 정상 발급되어야 함"
    );
    assert_record_state(repo, &mixed_case, &brk_id, InstrumentMappingStatus::Active).await;
    assert_eq!(
        repo.resolve(&mixed_variant).await,
        Err(mapping_err(InstrumentMappingError::IdentityUncertain)),
        "대소문자만 다른 심볼은 IdentityUncertain이어야 함"
    );
    assert_eq!(
        repo.retire(&unrelated).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "무관한 원본 폐지는 Missing이어야 함"
    );
    assert_eq!(
        repo.retire(original).await,
        Ok(original_id.clone()),
        "원본 폐지가 성공해야 함"
    );
    assert_all_uncertain(repo, overlaps, &relist_id).await;
    assert_eq!(
        repo.ensure(&unrelated, &relist_id).await,
        Ok(MappingAssignment::Issued(relist_id)),
        "거부된 후보는 소비되지 않아야 함"
    );
}

pub async fn uncertain_overlaps_and_case_variants<P: InstrumentMappingRepository>(repo: &P) {
    let Some(original) = parse_source(KnownMarket::Us, "AAPL", "US0378331005") else {
        return;
    };
    let Some(other_market) = parse_source(KnownMarket::Kr, "AAPL", "KR0000000001") else {
        return;
    };
    let (Some(us_id), Some(unused_id)) = (parse_id("opaque-us"), parse_id("opaque-unused")) else {
        return;
    };
    let Some(overlaps) = [
        (KnownMarket::Us, "AAPL", "US0000000002"),
        (KnownMarket::Us, "APLE", "US0378331005"),
        (KnownMarket::Kr, "005930", "US0378331005"),
        (KnownMarket::Us, "aapl", "US0378331005"),
        (KnownMarket::Us, "aapl", "US0000000003"),
        (KnownMarket::Us, "AAPL", "us0378331005"),
        (KnownMarket::Us, "DIFF", "us0378331005"),
    ]
    .into_iter()
    .map(|(market, sym, isin)| parse_source(market, sym, isin))
    .collect::<Option<Vec<_>>>() else {
        return;
    };

    assert_eq!(
        repo.ensure(&original, &us_id).await,
        Ok(MappingAssignment::Issued(us_id.clone())),
        "원본 US AAPL 발급"
    );
    assert_all_uncertain(repo, &overlaps, &unused_id).await;
    assert_eq!(
        repo.ensure(&other_market, &unused_id).await,
        Ok(MappingAssignment::Issued(unused_id.clone())),
        "다른 시장의 같은 심볼은 다른 ISIN·후보로 발급 가능해야 함"
    );
    assert_eq!(
        repo.resolve(&original).await,
        Ok(us_id.clone()),
        "원본은 겹침 실패 뒤에도 보존되어야 함"
    );
    assert_eq!(
        repo.resolve(&other_market).await,
        Ok(unused_id),
        "다른 시장 종목이 독립적으로 조회되어야 함"
    );
    verify_mixed_case_and_relisting(repo, &original, &us_id, &overlaps).await;
}

pub async fn candidate_collisions_fail_closed<P: InstrumentMappingRepository>(repo: &P) {
    let Some(first) = parse_source(KnownMarket::Kr, "005930", "KR7005930003") else {
        return;
    };
    let Some(second) = parse_source(KnownMarket::Us, "AAPL", "US0378331005") else {
        return;
    };
    let Some(third) = parse_source(KnownMarket::Us, "MSFT", "US5949181045") else {
        return;
    };
    let (Some(id1), Some(id2)) = (parse_id("opaque-1"), parse_id("opaque-2")) else {
        return;
    };

    assert_eq!(
        repo.ensure(&first, &id1).await,
        Ok(MappingAssignment::Issued(id1.clone())),
        "첫 원본 발급"
    );
    assert_eq!(
        repo.ensure(&second, &id1).await,
        Err(mapping_err(InstrumentMappingError::IdAlreadyUsed)),
        "이미 쓰인 후보 ID는 IdAlreadyUsed로 거부되어야 함"
    );
    assert_eq!(
        repo.resolve(&second).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "후보 충돌 뒤 둘째 원본은 미등록이어야 함"
    );
    assert_eq!(
        repo.lookup(&second).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "후보 충돌 뒤 둘째 원본의 lookup은 Missing이어야 함"
    );
    assert_record_state(repo, &first, &id1, InstrumentMappingStatus::Active).await;
    assert_eq!(
        repo.ensure(&second, &id2).await,
        Ok(MappingAssignment::Issued(id2)),
        "새 후보로 재시도하면 발급되어야 함"
    );
    assert_eq!(repo.retire(&first).await, Ok(id1.clone()), "첫 원본 폐지");
    assert_eq!(
        repo.ensure(&third, &id1).await,
        Err(mapping_err(InstrumentMappingError::IdAlreadyUsed)),
        "폐지된 ID 재사용은 IdAlreadyUsed로 거부되어야 함"
    );
    assert_eq!(
        repo.resolve(&third).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "폐지 ID 충돌 뒤 셋째 원본은 미등록이어야 함"
    );
}

pub async fn dropping_unpolled_writes_does_not_mutate<P: InstrumentMappingRepository>(repo: &P) {
    let Some(original) = parse_source(KnownMarket::Kr, "005930", "KR7005930003") else {
        return;
    };
    let Some(candidate) = parse_id("opaque-1") else {
        return;
    };
    {
        let _pending = repo.ensure(&original, &candidate);
    }
    assert_eq!(
        repo.resolve(&original).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "폴링되지 않은 ensure는 저장소를 바꾸면 안 됨"
    );
    assert_eq!(
        repo.ensure(&original, &candidate).await,
        Ok(MappingAssignment::Issued(candidate.clone())),
        "폴링된 ensure는 발급되어야 함"
    );
    {
        let _pending = repo.retire(&original);
    }
    assert_eq!(
        repo.resolve(&original).await,
        Ok(candidate),
        "폴링되지 않은 retire는 저장소를 바꾸면 안 됨"
    );
}

fn related(left: u8, right: u8) -> bool {
    [
        (0, 2),
        (2, 0),
        (0, 3),
        (3, 0),
        (0, 5),
        (5, 0),
        (2, 5),
        (5, 2),
    ]
    .contains(&(left, right))
}

struct ReferenceModel {
    sources: Vec<InstrumentSource>,
    candidates: Vec<InstrumentId>,
    entries: BTreeMap<u8, (u8, InstrumentMappingStatus)>,
}

impl ReferenceModel {
    fn build(salt: u16) -> Option<Self> {
        let tag = salt.to_string();
        let sources = [
            (
                KnownMarket::Us,
                ["S", &tag, "A"].concat(),
                ["I", &tag, "A"].concat(),
            ),
            (
                KnownMarket::Us,
                ["S", &tag, "B"].concat(),
                ["I", &tag, "B"].concat(),
            ),
            (
                KnownMarket::Us,
                ["S", &tag, "A"].concat(),
                ["I", &tag, "C"].concat(),
            ),
            (
                KnownMarket::Us,
                ["S", &tag, "C"].concat(),
                ["I", &tag, "A"].concat(),
            ),
            (
                KnownMarket::Kr,
                ["S", &tag, "A"].concat(),
                ["I", &tag, "D"].concat(),
            ),
            (
                KnownMarket::Us,
                ["s", &tag, "a"].concat(),
                ["I", &tag, "E"].concat(),
            ),
        ]
        .into_iter()
        .map(|(market, sym, isin)| parse_source(market, &sym, &isin))
        .collect::<Option<Vec<_>>>()?;
        let candidates = [
            ["cand-", &tag, "-0"].concat(),
            ["cand-", &tag, "-1"].concat(),
            ["cand-", &tag, "-2"].concat(),
        ]
        .into_iter()
        .map(|value| parse_id(&value))
        .collect::<Option<Vec<_>>>()?;
        Some(Self {
            sources,
            candidates,
            entries: BTreeMap::new(),
        })
    }

    fn source(&self, index: u8) -> Option<&InstrumentSource> {
        let item = self.sources.get(usize::from(index));
        assert!(item.is_some(), "참조 모델 원본 인덱스가 범위 안이어야 함");
        item
    }

    fn candidate(&self, index: u8) -> Option<&InstrumentId> {
        let item = self.candidates.get(usize::from(index));
        assert!(item.is_some(), "참조 모델 후보 인덱스가 범위 안이어야 함");
        item
    }

    fn is_uncertain(&self, index: u8) -> bool {
        self.entries.keys().any(|other| related(*other, index))
    }

    fn record(
        &self,
        source_index: u8,
        assigned: u8,
        status: InstrumentMappingStatus,
    ) -> Option<InstrumentMappingRecord> {
        Some(InstrumentMappingRecord::new(
            self.source(source_index)?.clone(),
            self.candidate(assigned)?.clone(),
            status,
        ))
    }

    fn expected_ensure(
        &mut self,
        source_index: u8,
        candidate_index: u8,
    ) -> Option<Result<MappingAssignment, MappingStoreError>> {
        Some(match self.entries.get(&source_index).copied() {
            Some((assigned, InstrumentMappingStatus::Active)) => Ok(MappingAssignment::Existing(
                self.candidate(assigned)?.clone(),
            )),
            Some((_, InstrumentMappingStatus::Retired)) => {
                Err(mapping_err(InstrumentMappingError::Retired))
            }
            None if self.is_uncertain(source_index) => {
                Err(mapping_err(InstrumentMappingError::IdentityUncertain))
            }
            None if self
                .entries
                .values()
                .any(|(assigned, _)| *assigned == candidate_index) =>
            {
                Err(mapping_err(InstrumentMappingError::IdAlreadyUsed))
            }
            None => {
                self.entries.insert(
                    source_index,
                    (candidate_index, InstrumentMappingStatus::Active),
                );
                Ok(MappingAssignment::Issued(
                    self.candidate(candidate_index)?.clone(),
                ))
            }
        })
    }

    fn expected_retire(
        &mut self,
        source_index: u8,
    ) -> Option<Result<InstrumentId, MappingStoreError>> {
        Some(match self.entries.get(&source_index).copied() {
            Some((assigned, _)) => {
                self.entries
                    .insert(source_index, (assigned, InstrumentMappingStatus::Retired));
                Ok(self.candidate(assigned)?.clone())
            }
            None if self.is_uncertain(source_index) => {
                Err(mapping_err(InstrumentMappingError::IdentityUncertain))
            }
            None => Err(mapping_err(InstrumentMappingError::Missing)),
        })
    }

    fn expected_resolve(
        &self,
        source_index: u8,
    ) -> Option<Result<InstrumentId, MappingStoreError>> {
        Some(match self.entries.get(&source_index).copied() {
            Some((assigned, InstrumentMappingStatus::Active)) => {
                Ok(self.candidate(assigned)?.clone())
            }
            Some((_, InstrumentMappingStatus::Retired)) => {
                Err(mapping_err(InstrumentMappingError::Retired))
            }
            None if self.is_uncertain(source_index) => {
                Err(mapping_err(InstrumentMappingError::IdentityUncertain))
            }
            None => Err(mapping_err(InstrumentMappingError::Missing)),
        })
    }

    fn expected_lookup(
        &self,
        source_index: u8,
    ) -> Option<Result<InstrumentMappingRecord, MappingStoreError>> {
        Some(match self.entries.get(&source_index).copied() {
            Some((assigned, status)) => Ok(self.record(source_index, assigned, status)?),
            None if self.is_uncertain(source_index) => {
                Err(mapping_err(InstrumentMappingError::IdentityUncertain))
            }
            None => Err(mapping_err(InstrumentMappingError::Missing)),
        })
    }

    fn expected_for_id(
        &self,
        candidate_index: u8,
    ) -> Option<Result<Vec<InstrumentMappingRecord>, MappingStoreError>> {
        Some(
            match self
                .entries
                .iter()
                .find(|(_, (assigned, _))| *assigned == candidate_index)
            {
                Some((source_index, (assigned, status))) => {
                    Ok(vec![self.record(*source_index, *assigned, *status)?])
                }
                None => Err(mapping_err(InstrumentMappingError::Missing)),
            },
        )
    }
}

async fn step_reference_model<P: InstrumentMappingRepository>(
    repo: &P,
    model: &mut ReferenceModel,
    step: (u8, u8, u8),
) {
    let (operation, source_index, candidate_index) = step;
    let (Some(src), Some(cand)) = (
        model.source(source_index).cloned(),
        model.candidate(candidate_index).cloned(),
    ) else {
        return;
    };
    match operation {
        0 => {
            if let Some(expected) = model.expected_ensure(source_index, candidate_index) {
                assert_eq!(
                    repo.ensure(&src, &cand).await,
                    expected,
                    "ensure 결과가 독립 참조 모델과 일치해야 함"
                );
            }
        }
        1 => {
            if let Some(expected) = model.expected_retire(source_index) {
                assert_eq!(
                    repo.retire(&src).await,
                    expected,
                    "retire 결과가 독립 참조 모델과 일치해야 함"
                );
            }
        }
        _ => {
            if let Some(expected) = model.expected_resolve(source_index) {
                assert_eq!(
                    repo.resolve(&src).await,
                    expected,
                    "resolve 결과가 독립 참조 모델과 일치해야 함"
                );
            }
        }
    }
    if let Some(expected_lookup) = model.expected_lookup(source_index) {
        assert_eq!(
            repo.lookup(&src).await,
            expected_lookup,
            "lookup 결과가 독립 참조 모델과 일치해야 함"
        );
    }
    if let Some(expected_for_id) = model.expected_for_id(candidate_index) {
        assert_eq!(
            repo.for_id(&cand).await,
            expected_for_id,
            "for_id 결과가 독립 참조 모델과 일치해야 함"
        );
    }
}

pub async fn reference_model_sequence<P: InstrumentMappingRepository>(
    repo: &P,
    salt: u16,
    operations: &[(u8, u8, u8)],
) {
    let Some(mut model) = ReferenceModel::build(salt) else {
        return;
    };
    for &step in operations {
        step_reference_model(repo, &mut model, step).await;
    }
}

fn race_pair<T, L, R>(left_fn: L, right_fn: R) -> Option<(T, T)>
where
    T: Send,
    L: FnOnce(Arc<Barrier>) -> T + Send,
    R: FnOnce(Arc<Barrier>) -> T + Send,
{
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|scope| {
        let left_barrier = Arc::clone(&barrier);
        let left_handle = scope.spawn(move || left_fn(left_barrier));
        let right_barrier = Arc::clone(&barrier);
        let right_handle = scope.spawn(move || right_fn(right_barrier));
        let left_joined = left_handle.join();
        assert!(left_joined.is_ok(), "왼쪽 경쟁 스레드가 패닉하면 안 됨");
        let right_joined = right_handle.join();
        assert!(right_joined.is_ok(), "오른쪽 경쟁 스레드가 패닉하면 안 됨");
        match (left_joined, right_joined) {
            (Ok(first), Ok(second)) => Some((first, second)),
            (Ok(_) | Err(_), Ok(_) | Err(_)) => None,
        }
    })
}

pub fn concurrent_same_source_has_one_winner<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository,
    R: ContractRuntime,
{
    let Some(original) = parse_source(KnownMarket::Kr, "005930", "KR7005930003") else {
        return;
    };
    let (Some(id1), Some(id2)) = (parse_id("opaque-1"), parse_id("opaque-2")) else {
        return;
    };
    let Some((first, second)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&original, &id1))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&original, &id2))
        },
    ) else {
        return;
    };
    let winner = match (&first, &second) {
        (Ok(MappingAssignment::Issued(issued)), Ok(MappingAssignment::Existing(existing)))
        | (Ok(MappingAssignment::Existing(existing)), Ok(MappingAssignment::Issued(issued))) => {
            assert_eq!(
                issued, existing,
                "동시 발급 승자와 기존 반환 ID는 같아야 함"
            );
            Some(issued.clone())
        }
        _ => None,
    };
    assert!(
        winner.is_some(),
        "동일 원본 경쟁은 Issued 1건과 Existing 1건이어야 함: ({first:?}, {second:?})"
    );
    if let Some(id) = winner {
        assert_eq!(
            runtime.block_on(left.resolve(&original)),
            Ok(id.clone()),
            "왼쪽 연결이 승자 ID를 조회해야 함"
        );
        assert_eq!(
            runtime.block_on(right.resolve(&original)),
            Ok(id),
            "오른쪽 연결이 같은 승자 ID를 조회해야 함"
        );
    }
}

pub fn concurrent_distinct_sources_same_candidate_has_one_winner<P, R>(
    left: &P,
    right: &P,
    runtime: &R,
) where
    P: InstrumentMappingRepository,
    R: ContractRuntime,
{
    let Some(first) = parse_source(KnownMarket::Kr, "005930", "KR7005930003") else {
        return;
    };
    let Some(second) = parse_source(KnownMarket::Us, "AAPL", "US0378331005") else {
        return;
    };
    let Some(shared) = parse_id("opaque-shared") else {
        return;
    };
    let Some((left_result, right_result)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&first, &shared))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&second, &shared))
        },
    ) else {
        return;
    };
    let issued = Ok(MappingAssignment::Issued(shared.clone()));
    let collision = Err(mapping_err(InstrumentMappingError::IdAlreadyUsed));
    assert!(
        (left_result == issued && right_result == collision)
            || (left_result == collision && right_result == issued),
        "같은 후보 경쟁은 Issued 1건과 IdAlreadyUsed 1건이어야 함: ({left_result:?}, {right_result:?})"
    );
    let outcomes = [
        runtime.block_on(left.resolve(&first)),
        runtime.block_on(right.resolve(&second)),
    ];
    assert_eq!(
        outcomes
            .iter()
            .filter(|entry| **entry == Ok(shared.clone()))
            .count(),
        1,
        "한 원본만 승자 ID를 가져야 함"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|entry| **entry == Err(mapping_err(InstrumentMappingError::Missing)))
            .count(),
        1,
        "다른 원본은 미등록 상태로 남아야 함"
    );
}

pub fn concurrent_overlapping_sources_has_one_winner<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository,
    R: ContractRuntime,
{
    let Some(first) = parse_source(KnownMarket::Us, "AAPL", "US0378331005") else {
        return;
    };
    let Some(overlapping) = parse_source(KnownMarket::Us, "aapl", "US0000000002") else {
        return;
    };
    let (Some(left_id), Some(right_id)) = (parse_id("opaque-left"), parse_id("opaque-right"))
    else {
        return;
    };
    let Some((left_result, right_result)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&first, &left_id))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&overlapping, &right_id))
        },
    ) else {
        return;
    };
    let uncertain_ensure = Err(mapping_err(InstrumentMappingError::IdentityUncertain));
    let uncertain_resolve = Err(mapping_err(InstrumentMappingError::IdentityUncertain));
    match (&left_result, &right_result) {
        (Ok(MappingAssignment::Issued(winner)), right_err) if *right_err == uncertain_ensure => {
            assert_eq!(
                runtime.block_on(right.resolve(&first)),
                Ok(winner.clone()),
                "승자 원본은 다른 연결에서도 조회되어야 함"
            );
            assert_eq!(
                runtime.block_on(left.resolve(&overlapping)),
                uncertain_resolve,
                "패자 겹침 원본은 IdentityUncertain이어야 함"
            );
        }
        (left_err, Ok(MappingAssignment::Issued(winner))) if *left_err == uncertain_ensure => {
            assert_eq!(
                runtime.block_on(left.resolve(&overlapping)),
                Ok(winner.clone()),
                "승자 원본은 다른 연결에서도 조회되어야 함"
            );
            assert_eq!(
                runtime.block_on(right.resolve(&first)),
                uncertain_resolve,
                "패자 겹침 원본은 IdentityUncertain이어야 함"
            );
        }
        _ => assert_eq!(
            left_result, uncertain_ensure,
            "겹침 원본 경쟁은 Issued 1건과 IdentityUncertain 1건이어야 함: ({left_result:?}, {right_result:?})"
        ),
    }
}

pub fn concurrent_ensure_and_retire_never_reactivates<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository,
    R: ContractRuntime,
{
    let Some(original) = parse_source(KnownMarket::Kr, "005930", "KR7005930003") else {
        return;
    };
    let (Some(id1), Some(id2)) = (parse_id("opaque-1"), parse_id("opaque-2")) else {
        return;
    };
    assert_eq!(
        runtime.block_on(left.ensure(&original, &id1)),
        Ok(MappingAssignment::Issued(id1.clone())),
        "폐지 경쟁 전 초기 발급"
    );
    let Some((ensured, retired)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&original, &id2))
        },
        |barrier| {
            barrier.wait();
            runtime
                .block_on(right.retire(&original))
                .map(MappingAssignment::Existing)
        },
    ) else {
        return;
    };
    assert!(
        ensured == Ok(MappingAssignment::Existing(id1.clone()))
            || ensured == Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지 경쟁 중 ensure는 Existing 또는 Retired만 가능해야 함: {ensured:?}"
    );
    assert_eq!(
        retired,
        Ok(MappingAssignment::Existing(id1.clone())),
        "폐지는 기존 ID를 반환해야 함"
    );
    assert_eq!(
        runtime.block_on(left.resolve(&original)),
        Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지 뒤 왼쪽 연결에서 재활성화되면 안 됨"
    );
    assert_eq!(
        runtime.block_on(right.resolve(&original)),
        Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지 뒤 오른쪽 연결에서 재활성화되면 안 됨"
    );
    assert_eq!(
        runtime.block_on(right.lookup(&original)),
        Ok(InstrumentMappingRecord::new(
            original,
            id1,
            InstrumentMappingStatus::Retired
        )),
        "폐지 이력이 보존되어야 함"
    );
}

pub fn concurrent_retire_is_idempotent<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository,
    R: ContractRuntime,
{
    let Some(original) = parse_source(KnownMarket::Us, "AAPL", "US0378331005") else {
        return;
    };
    let Some(id1) = parse_id("opaque-1") else {
        return;
    };
    assert_eq!(
        runtime.block_on(left.ensure(&original, &id1)),
        Ok(MappingAssignment::Issued(id1.clone())),
        "동시 폐지 전 초기 발급"
    );
    let Some((first, second)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.retire(&original))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.retire(&original))
        },
    ) else {
        return;
    };
    assert_eq!(first, Ok(id1.clone()), "첫 동시 폐지는 기존 ID 반환");
    assert_eq!(second, Ok(id1.clone()), "둘째 동시 폐지도 같은 ID 반환");
    assert_eq!(
        runtime.block_on(left.for_id(&id1)),
        Ok(vec![InstrumentMappingRecord::new(
            original,
            id1,
            InstrumentMappingStatus::Retired
        )]),
        "동시 폐지 뒤 단일 Retired 이력이 남아야 함"
    );
}
