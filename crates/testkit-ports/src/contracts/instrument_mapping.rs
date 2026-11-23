use application::{
    InstrumentIdIssuer, InstrumentMappingHistoryRepository, InstrumentMappingRepository,
    MappingStoreError,
};
use domain::{
    CalendarDate, CalendarYear, CorporateAction, CorporateActionId, CorporateActionKind,
    DartReceiptNo, DayNumber, DisclosureKey, IndependentInstrumentIdIssuer, InstrumentId,
    InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus, InstrumentSource,
    IsinCode, IssuedInstrumentId, KindReceiptNo, KnownMarket, MappingAssignment,
    MappingHistoryEvent, MonthNumber, Nanosecond, OffsetSeconds, ReceiptNo, SecEdgarReceiptNo,
    ShareCount, ShareRatio, Symbol, Timestamp, UnixSeconds, UntrustedText, UserReason,
};
use std::{
    collections::{BTreeMap, BTreeSet},
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

fn issue_slot(slot: u16) -> Option<IssuedInstrumentId> {
    let mut issuer = IndependentInstrumentIdIssuer::new();
    for _ in 0..slot {
        let skipped = issuer.issue();
        assert!(skipped.is_ok(), "계약 증인 발급원 건너뛰기가 유효해야 함");
        if skipped.is_err() {
            return None;
        }
    }
    let candidate = issuer.issue();
    assert!(candidate.is_ok(), "계약 증인 내부 ID 발급이 유효해야 함");
    candidate.ok()
}

fn slot_id(slot: u16) -> Option<InstrumentId> {
    issue_slot(slot).map(|issued| issued.as_instrument_id().clone())
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
    slots: (u16, u16),
) {
    let (first_slot, second_slot) = slots;
    let (Some(first_id), Some(second_id)) = (slot_id(first_slot), slot_id(second_slot)) else {
        return;
    };
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
    let (Some(cand_same), Some(cand_next), Some(cand_ind), Some(cand_reuse)) = (
        issue_slot(first_slot),
        issue_slot(second_slot),
        issue_slot(second_slot),
        issue_slot(first_slot),
    ) else {
        return;
    };
    assert_eq!(
        repo.ensure(original, cand_same).await,
        Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지된 원본의 같은 후보 재발급은 Retired여야 함"
    );
    assert_eq!(
        repo.ensure(original, cand_next).await,
        Err(mapping_err(InstrumentMappingError::Retired)),
        "폐지된 원본의 새 후보 재발급은 Retired여야 함"
    );
    assert_record_state(repo, original, &first_id, InstrumentMappingStatus::Retired).await;
    assert_eq!(
        repo.ensure(independent, cand_ind).await,
        Ok(MappingAssignment::Issued(second_id)),
        "무시된 후보는 다른 원본에서 정상 발급 가능해야 함"
    );
    assert_eq!(
        repo.ensure(third, cand_reuse).await,
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
    let (Some(first_id), Some(cand_1), Some(cand_2)) = (slot_id(0), issue_slot(0), issue_slot(1))
    else {
        return;
    };

    assert_missing(repo, &original, &first_id).await;
    assert_eq!(
        repo.ensure(&original, cand_1).await,
        Ok(MappingAssignment::Issued(first_id.clone())),
        "최초 발급은 Issued를 반환해야 함"
    );
    assert_eq!(
        repo.resolve(&original).await,
        Ok(first_id.clone()),
        "발급 직후 resolve는 기존 ID를 반환해야 함"
    );
    assert_eq!(
        repo.ensure(&original, cand_2).await,
        Ok(MappingAssignment::Existing(first_id.clone())),
        "기존 활성 원본은 후보 ID를 무시하고 Existing을 반환해야 함"
    );
    assert_record_state(repo, &original, &first_id, InstrumentMappingStatus::Active).await;
    assert_retired_and_reused_rejection(repo, &original, &independent, &third, (0, 1)).await;
}

async fn assert_all_uncertain<P: InstrumentMappingRepository>(
    repo: &P,
    candidates: &[InstrumentSource],
    unused_slot: u16,
) {
    for candidate in candidates {
        let Some(issued) = issue_slot(unused_slot) else {
            return;
        };
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
            repo.ensure(candidate, issued).await,
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
    let (Some(brk_cand), Some(relist_cand)) = (issue_slot(2), issue_slot(3)) else {
        return;
    };
    let brk_id = brk_cand.as_instrument_id().clone();
    let relist_id = relist_cand.as_instrument_id().clone();

    assert_eq!(
        repo.ensure(&mixed_case, brk_cand).await,
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
    assert_all_uncertain(repo, overlaps, 3).await;
    assert_eq!(
        repo.ensure(&unrelated, relist_cand).await,
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
    let (Some(us_cand), Some(unused_cand)) = (issue_slot(0), issue_slot(1)) else {
        return;
    };
    let us_id = us_cand.as_instrument_id().clone();
    let unused_id = unused_cand.as_instrument_id().clone();
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
        repo.ensure(&original, us_cand).await,
        Ok(MappingAssignment::Issued(us_id.clone())),
        "원본 US AAPL 발급"
    );
    assert_all_uncertain(repo, &overlaps, 1).await;
    assert_eq!(
        repo.ensure(&other_market, unused_cand).await,
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
    let (Some(cand1), Some(cand1_dup), Some(cand2), Some(cand1_retired)) =
        (issue_slot(0), issue_slot(0), issue_slot(1), issue_slot(0))
    else {
        return;
    };
    let id1 = cand1.as_instrument_id().clone();
    let id2 = cand2.as_instrument_id().clone();

    assert_eq!(
        repo.ensure(&first, cand1).await,
        Ok(MappingAssignment::Issued(id1.clone())),
        "첫 원본 발급"
    );
    assert_eq!(
        repo.ensure(&second, cand1_dup).await,
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
        repo.ensure(&second, cand2).await,
        Ok(MappingAssignment::Issued(id2)),
        "새 후보로 재시도하면 발급되어야 함"
    );
    assert_eq!(repo.retire(&first).await, Ok(id1), "첫 원본 폐지");
    assert_eq!(
        repo.ensure(&third, cand1_retired).await,
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
    let (Some(cand_unpolled), Some(cand_polled)) = (issue_slot(0), issue_slot(0)) else {
        return;
    };
    let expected_id = cand_polled.as_instrument_id().clone();
    {
        let _pending = repo.ensure(&original, cand_unpolled);
    }
    assert_eq!(
        repo.resolve(&original).await,
        Err(mapping_err(InstrumentMappingError::Missing)),
        "폴링되지 않은 ensure는 저장소를 바꾸면 안 됨"
    );
    assert_eq!(
        repo.ensure(&original, cand_polled).await,
        Ok(MappingAssignment::Issued(expected_id.clone())),
        "폴링된 ensure는 발급되어야 함"
    );
    {
        let _pending = repo.retire(&original);
    }
    assert_eq!(
        repo.resolve(&original).await,
        Ok(expected_id),
        "폴링되지 않은 retire는 저장소를 바꾸면 안 됨"
    );
}

async fn issue_next_candidate<I: InstrumentIdIssuer>(issuer: &I) -> Option<IssuedInstrumentId> {
    let result = issuer.issue().await;
    assert!(result.is_ok(), "발급원 호출이 성공해야 함");
    result.ok()
}

async fn seed_preexisting_retired_and_active<P: InstrumentMappingRepository>(
    repo: &P,
) -> Option<(InstrumentId, InstrumentId)> {
    let (Some(retired_src), Some(active_src), Some(external_first), Some(external_second)) = (
        parse_source(KnownMarket::Kr, "005930", "KR7005930003"),
        parse_source(KnownMarket::Us, "NVDA", "US67066G1040"),
        issue_slot(0),
        issue_slot(1),
    ) else {
        return None;
    };
    let external_first_id = external_first.as_instrument_id().clone();
    let external_second_id = external_second.as_instrument_id().clone();
    assert_eq!(
        repo.ensure(&retired_src, external_first).await,
        Ok(MappingAssignment::Issued(external_first_id.clone())),
        "사전 등록 폐지 대상 발급"
    );
    assert_eq!(
        repo.retire(&retired_src).await,
        Ok(external_first_id.clone()),
        "사전 등록 원본 폐지"
    );
    assert_eq!(
        repo.ensure(&active_src, external_second).await,
        Ok(MappingAssignment::Issued(external_second_id.clone())),
        "사전 등록 활성 원본 발급"
    );
    Some((external_first_id, external_second_id))
}

pub async fn issuer_persists_uniqueness_and_skips_retired_and_existing<I, P>(issuer: &I, repo: &P)
where
    I: InstrumentIdIssuer,
    P: InstrumentMappingRepository,
{
    let Some((external_first_id, external_second_id)) =
        seed_preexisting_retired_and_active(repo).await
    else {
        return;
    };
    let (Some(second_src), Some(third_src)) = (
        parse_source(KnownMarket::Us, "AAPL", "US0378331005"),
        parse_source(KnownMarket::Us, "MSFT", "US5949181045"),
    ) else {
        return;
    };

    let Some(second_cand) = issue_next_candidate(issuer).await else {
        return;
    };
    let second_id = second_cand.as_instrument_id().clone();
    assert_ne!(
        second_id, external_first_id,
        "폐지된 기존 ID는 다시 발급되면 안 됨"
    );
    assert_ne!(
        second_id, external_second_id,
        "활성 기존 ID는 다시 발급되면 안 됨"
    );
    assert_eq!(
        repo.ensure(&second_src, second_cand).await,
        Ok(MappingAssignment::Issued(second_id.clone())),
        "발급원이 준 후보로 등록 성공"
    );
    assert_eq!(
        repo.retire(&second_src).await,
        Ok(second_id.clone()),
        "둘째 원본 폐지 성공"
    );
    {
        let _unpolled_issue = issuer.issue();
    }

    let Some(third_cand) = issue_next_candidate(issuer).await else {
        return;
    };
    let third_id = third_cand.as_instrument_id().clone();
    assert_ne!(third_id, external_first_id, "셋째 ID는 첫째와 달라야 함");
    assert_ne!(third_id, external_second_id, "셋째 ID는 둘째와 달라야 함");
    assert_ne!(third_id, second_id, "셋째 ID는 직전 폐지 ID와 달라야 함");
    assert_eq!(
        repo.ensure(&third_src, third_cand).await,
        Ok(MappingAssignment::Issued(third_id)),
        "미폴링 드롭과 폐지 뒤 새 발급 후보로 등록 성공"
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
    candidate_slots: [u16; 3],
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
        let base_slot = salt.checked_mul(4)?;
        let candidate_slots = [
            base_slot,
            base_slot.checked_add(1)?,
            base_slot.checked_add(2)?,
        ];
        let candidates = candidate_slots
            .into_iter()
            .map(slot_id)
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            sources,
            candidate_slots,
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

    fn issued_candidate(&self, index: u8) -> Option<IssuedInstrumentId> {
        let slot = self.candidate_slots.get(usize::from(index)).copied();
        assert!(slot.is_some(), "참조 모델 후보 슬롯이 범위 안이어야 함");
        issue_slot(slot?)
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
            Some((_, InstrumentMappingStatus::Superseded)) => {
                Err(mapping_err(InstrumentMappingError::Superseded))
            }
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
            Some((_, InstrumentMappingStatus::Superseded)) => {
                Err(mapping_err(InstrumentMappingError::Superseded))
            }
            Some((
                assigned,
                InstrumentMappingStatus::Active | InstrumentMappingStatus::Retired,
            )) => {
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
            Some((_, InstrumentMappingStatus::Superseded)) => {
                Err(mapping_err(InstrumentMappingError::Superseded))
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
    let (Some(src), Some(cand_id)) = (
        model.source(source_index).cloned(),
        model.candidate(candidate_index).cloned(),
    ) else {
        return;
    };
    match operation {
        0 => {
            if let (Some(issued), Some(expected)) = (
                model.issued_candidate(candidate_index),
                model.expected_ensure(source_index, candidate_index),
            ) {
                assert_eq!(
                    repo.ensure(&src, issued).await,
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
            repo.for_id(&cand_id).await,
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
    let (Some(cand1), Some(cand2)) = (issue_slot(0), issue_slot(1)) else {
        return;
    };
    let Some((first, second)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&original, cand1))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&original, cand2))
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
    let (Some(left_cand), Some(right_cand)) = (issue_slot(0), issue_slot(0)) else {
        return;
    };
    let shared = left_cand.as_instrument_id().clone();
    let Some((left_result, right_result)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&first, left_cand))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&second, right_cand))
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
    let (Some(left_cand), Some(right_cand)) = (issue_slot(0), issue_slot(1)) else {
        return;
    };
    let left_id = left_cand.as_instrument_id().clone();
    let right_id = right_cand.as_instrument_id().clone();
    let Some((left_result, right_result)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&first, left_cand))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&overlapping, right_cand))
        },
    ) else {
        return;
    };
    let uncertain_ensure = Err(mapping_err(InstrumentMappingError::IdentityUncertain));
    let uncertain_resolve = Err(mapping_err(InstrumentMappingError::IdentityUncertain));
    let winner = match (&left_result, &right_result) {
        (Ok(MappingAssignment::Issued(issued)), right_err)
            if *issued == left_id && *right_err == uncertain_ensure =>
        {
            Some((&first, &overlapping, issued.clone()))
        }
        (left_err, Ok(MappingAssignment::Issued(issued)))
            if *left_err == uncertain_ensure && *issued == right_id =>
        {
            Some((&overlapping, &first, issued.clone()))
        }
        _ => None,
    };
    assert!(
        winner.is_some(),
        "겹침 원본 경쟁은 Issued 1건과 IdentityUncertain 1건이어야 함: ({left_result:?}, {right_result:?})"
    );
    if let Some((winner_source, loser_source, winner_id)) = winner {
        for peer in [left, right] {
            assert_eq!(
                runtime.block_on(peer.resolve(winner_source)),
                Ok(winner_id.clone()),
                "승자 원본은 양쪽 연결에서 승자 ID로 조회되어야 함"
            );
            assert_eq!(
                runtime.block_on(peer.resolve(loser_source)),
                uncertain_resolve,
                "패자 겹침 원본은 양쪽 연결에서 IdentityUncertain이어야 함"
            );
        }
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
    let (Some(cand1), Some(cand2)) = (issue_slot(0), issue_slot(1)) else {
        return;
    };
    let id1 = cand1.as_instrument_id().clone();
    assert_eq!(
        runtime.block_on(left.ensure(&original, cand1)),
        Ok(MappingAssignment::Issued(id1.clone())),
        "폐지 경쟁 전 초기 발급"
    );
    let Some((ensured, retired)) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.ensure(&original, cand2))
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
    for peer in [left, right] {
        assert_eq!(
            runtime.block_on(peer.resolve(&original)),
            Err(mapping_err(InstrumentMappingError::Retired)),
            "폐지 뒤 재활성화되면 안 됨"
        );
    }
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
    let Some(cand1) = issue_slot(0) else {
        return;
    };
    let id1 = cand1.as_instrument_id().clone();
    assert_eq!(
        runtime.block_on(left.ensure(&original, cand1)),
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

pub fn concurrent_issuer_never_duplicates<I, R>(left: &I, right: &I, runtime: &R)
where
    I: InstrumentIdIssuer,
    R: ContractRuntime,
{
    let Some((left_ids, right_ids)) = race_pair(
        |barrier| {
            barrier.wait();
            (0_u8..10_u8)
                .map(|_| {
                    let result = runtime.block_on(left.issue());
                    assert!(result.is_ok(), "왼쪽 동시 발급 성공");
                    result.ok().map(|issued| issued.as_instrument_id().clone())
                })
                .collect::<Option<Vec<_>>>()
        },
        |barrier| {
            barrier.wait();
            (0_u8..10_u8)
                .map(|_| {
                    let result = runtime.block_on(right.issue());
                    assert!(result.is_ok(), "오른쪽 동시 발급 성공");
                    result.ok().map(|issued| issued.as_instrument_id().clone())
                })
                .collect::<Option<Vec<_>>>()
        },
    ) else {
        return;
    };
    let (Some(left_batch), Some(right_batch)) = (left_ids, right_ids) else {
        return;
    };
    let mut seen = BTreeSet::new();
    for id in left_batch.into_iter().chain(right_batch) {
        assert!(seen.insert(id), "동시 발급에서 중복 ID가 나오면 안 됨");
    }
    assert_eq!(seen.len(), 20, "동시 발급 총 20건이 모두 고유해야 함");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryContractDataset {
    pub primary_id: InstrumentId,
    pub secondary_id: InstrumentId,
    pub quiet_id: InstrumentId,
    pub unknown_id: InstrumentId,
    pub records: Vec<InstrumentMappingRecord>,
    pub events: Vec<MappingHistoryEvent>,
}

fn parse_reason(text: &str) -> Option<UserReason> {
    let untrusted = UntrustedText::try_new(text);
    assert!(untrusted.is_ok(), "계약 증인 사유 텍스트가 유효해야 함");
    let parsed = untrusted
        .ok()
        .and_then(|value| UserReason::try_new(value).ok());
    assert!(parsed.is_some(), "계약 증인 사유가 유효해야 함");
    parsed
}

fn parse_timestamp(seconds: i64, nanos: u32, offset: i32) -> Option<Timestamp> {
    let parsed = Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    );
    assert!(parsed.is_ok(), "계약 증인 시각이 유효해야 함");
    parsed.ok()
}

fn parse_date(year: i32, month: u8, day: u8) -> Option<CalendarDate> {
    let parsed = CalendarDate::from_parts(
        CalendarYear::new(year),
        MonthNumber::new(month),
        DayNumber::new(day),
    );
    assert!(parsed.is_ok(), "계약 증인 일자가 유효해야 함");
    parsed.ok()
}

fn parse_disclosure_dart(raw: &str) -> Option<DisclosureKey> {
    let receipt = DartReceiptNo::try_new(raw);
    assert!(receipt.is_ok(), "계약 증인 DART 접수번호가 유효해야 함");
    receipt
        .ok()
        .map(|value| DisclosureKey::from_receipt(ReceiptNo::Dart(value)))
}

fn parse_disclosure_kind(raw: &str) -> Option<DisclosureKey> {
    let receipt = KindReceiptNo::try_new(raw);
    assert!(receipt.is_ok(), "계약 증인 KIND 접수번호가 유효해야 함");
    receipt
        .ok()
        .map(|value| DisclosureKey::from_receipt(ReceiptNo::Kind(value)))
}

fn parse_disclosure_sec(raw: &str) -> Option<DisclosureKey> {
    let receipt = SecEdgarReceiptNo::try_new(raw);
    assert!(receipt.is_ok(), "계약 증인 SEC 접수번호가 유효해야 함");
    receipt
        .ok()
        .map(|value| DisclosureKey::from_receipt(ReceiptNo::SecEdgar(value)))
}

fn build_contract_action(
    id: &InstrumentId,
    date: CalendarDate,
    kind: CorporateActionKind,
) -> Option<CorporateAction> {
    let action = CorporateAction::try_new(CorporateActionId::new(id.clone()), date, kind);
    assert!(action.is_ok(), "계약 증인 기업행위가 유효해야 함");
    action.ok()
}

fn build_primary_history_events(
    primary_id: &InstrumentId,
    us_prev: &InstrumentSource,
    us_next: &InstrumentSource,
    us_mistake: &InstrumentSource,
) -> Option<Vec<MappingHistoryEvent>> {
    let action_sym = build_contract_action(
        primary_id,
        parse_date(2026, 10, 28)?,
        CorporateActionKind::SymbolChange {
            from: Symbol::try_new("FB.A").ok()?,
            to: Symbol::try_new("META").ok()?,
        },
    )?;
    let action_split = build_contract_action(
        primary_id,
        parse_date(2026, 11, 2)?,
        CorporateActionKind::Split(ShareRatio::new(
            ShareCount::try_new(1).ok()?,
            ShareCount::try_new(10).ok()?,
        )),
    )?;
    Some(vec![
        MappingHistoryEvent::IdentifierChanged {
            instrument_id: primary_id.clone(),
            previous: us_prev.clone(),
            next: us_next.clone(),
            disclosure: parse_disclosure_sec("0001326801-26-000012")?,
            action: Box::new(action_sym),
            note: parse_reason("사명 및 심볼 변경 확인")?,
            confirmed_at: parse_timestamp(1_762_000_000, 123_456_789, -18_000)?,
        },
        MappingHistoryEvent::IdentifierChanged {
            instrument_id: primary_id.clone(),
            previous: us_next.clone(),
            next: us_mistake.clone(),
            disclosure: parse_disclosure_kind("20261102000001")?,
            action: Box::new(action_split),
            note: parse_reason("오연결 분할 기록")?,
            confirmed_at: parse_timestamp(1_761_000_000, 999_999_999, 0)?,
        },
        MappingHistoryEvent::LinkCorrected {
            instrument_id: primary_id.clone(),
            restored_previous: us_next.clone(),
            unlinked_next: us_mistake.clone(),
            disclosure: parse_disclosure_dart("20261103000002")?,
            note: parse_reason("오연결 정정 및 이전 활성 복구")?,
            corrected_at: parse_timestamp(1_760_000_000, 42, 32_400)?,
        },
    ])
}

fn build_secondary_history_events(
    secondary_id: &InstrumentId,
    kr_prev: &InstrumentSource,
    kr_next: &InstrumentSource,
) -> Option<Vec<MappingHistoryEvent>> {
    let action_reverse = build_contract_action(
        secondary_id,
        parse_date(2026, 11, 10)?,
        CorporateActionKind::ReverseSplit(ShareRatio::new(
            ShareCount::try_new(u64::MAX).ok()?,
            ShareCount::try_new(1).ok()?,
        )),
    )?;
    Some(vec![
        MappingHistoryEvent::IdentifierChanged {
            instrument_id: secondary_id.clone(),
            previous: kr_prev.clone(),
            next: kr_next.clone(),
            disclosure: parse_disclosure_dart("20261110000003")?,
            action: Box::new(action_reverse),
            note: parse_reason("액면병합 ISIN 변경")?,
            confirmed_at: parse_timestamp(1_759_000_000, 500_000, 32_400)?,
        },
        MappingHistoryEvent::RetirementCorrected {
            instrument_id: secondary_id.clone(),
            source: kr_next.clone(),
            disclosure: parse_disclosure_kind("20261111000004")?,
            note: parse_reason("오폐지 복구")?,
            corrected_at: parse_timestamp(1_758_000_000, 0, 32_400)?,
        },
    ])
}

#[must_use]
pub fn history_contract_dataset() -> Option<HistoryContractDataset> {
    let primary_id = slot_id(0)?;
    let secondary_id = slot_id(1)?;
    let quiet_id = slot_id(2)?;
    let unknown_id = slot_id(3)?;

    let us_prev = parse_source(KnownMarket::Us, "fb.A", "us30303m1027")?;
    let us_next = parse_source(KnownMarket::Us, "META", "us30303m1027")?;
    let us_mistake = parse_source(KnownMarket::Us, "MIST", "US30303M9999")?;
    let kr_prev = parse_source(KnownMarket::Kr, "005930", "KR7005930003")?;
    let kr_next = parse_source(KnownMarket::Kr, "005930", "KR7005930999")?;
    let quiet_src = parse_source(KnownMarket::Us, "NVDA", "US67066G1040")?;

    let mut events = build_primary_history_events(&primary_id, &us_prev, &us_next, &us_mistake)?;
    events.extend(build_secondary_history_events(
        &secondary_id,
        &kr_prev,
        &kr_next,
    )?);

    let records = vec![
        InstrumentMappingRecord::new(
            us_prev,
            primary_id.clone(),
            InstrumentMappingStatus::Superseded,
        ),
        InstrumentMappingRecord::new(us_next, primary_id.clone(), InstrumentMappingStatus::Active),
        InstrumentMappingRecord::new(
            kr_prev,
            secondary_id.clone(),
            InstrumentMappingStatus::Superseded,
        ),
        InstrumentMappingRecord::new(
            kr_next,
            secondary_id.clone(),
            InstrumentMappingStatus::Active,
        ),
        InstrumentMappingRecord::new(quiet_src, quiet_id.clone(), InstrumentMappingStatus::Active),
    ];

    Some(HistoryContractDataset {
        primary_id,
        secondary_id,
        quiet_id,
        unknown_id,
        records,
        events,
    })
}

fn event_timestamp(event: &MappingHistoryEvent) -> Timestamp {
    match event {
        MappingHistoryEvent::IdentifierChanged { confirmed_at, .. } => *confirmed_at,
        MappingHistoryEvent::RetirementCorrected { corrected_at, .. }
        | MappingHistoryEvent::LinkCorrected { corrected_at, .. } => *corrected_at,
    }
}

pub async fn empty_history_contract<P, H>(repo: &P, history_repo: &H)
where
    P: InstrumentMappingRepository,
    H: InstrumentMappingHistoryRepository,
{
    let Some(sample) = parse_source(KnownMarket::Kr, "005930", "KR7005930003") else {
        return;
    };
    let Some(candidate) = issue_slot(0) else {
        return;
    };
    let id = candidate.as_instrument_id().clone();

    assert_eq!(
        history_repo.history().await,
        Ok(Vec::new()),
        "초기 저장소의 전체 감사 이력은 비어 있어야 함"
    );
    assert_eq!(
        history_repo.history_for_id(&id).await,
        Ok(Vec::new()),
        "미등록 ID의 감사 이력은 비어 있어야 함"
    );
    assert_eq!(
        repo.ensure(&sample, candidate).await,
        Ok(MappingAssignment::Issued(id.clone())),
        "일반 등록 성공"
    );
    assert_eq!(repo.retire(&sample).await, Ok(id.clone()), "일반 폐지 성공");
    assert_eq!(
        history_repo.history().await,
        Ok(Vec::new()),
        "일반 등록·폐지 뒤에도 변경·정정 감사 이력은 비어 있어야 함"
    );
    assert_eq!(
        history_repo.history_for_id(&id).await,
        Ok(Vec::new()),
        "일반 등록·폐지 ID의 감사 이력은 비어 있어야 함"
    );
}

fn assert_events_match_exact(
    actual: Result<Vec<MappingHistoryEvent>, application::SystemPortError>,
    expected: &[MappingHistoryEvent],
    context: &str,
) {
    assert_eq!(actual, Ok(expected.to_vec()), "{context}");
    if let Ok(actual_events) = actual {
        for (got, want) in actual_events.iter().zip(expected.iter()) {
            let got_ts = event_timestamp(got);
            let want_ts = event_timestamp(want);
            assert_eq!(
                got_ts.offset_seconds(),
                want_ts.offset_seconds(),
                "{context}: 감사 이력 시각 오프셋이 보존되어야 함"
            );
            assert_eq!(
                got_ts.nanosecond(),
                want_ts.nanosecond(),
                "{context}: 감사 이력 시각 나노초가 보존되어야 함"
            );
        }
    }
}

pub async fn populated_history_contract<H>(history_repo: &H, dataset: &HistoryContractDataset)
where
    H: InstrumentMappingHistoryRepository,
{
    {
        let _unpolled_all = history_repo.history();
        let _unpolled_for_id = history_repo.history_for_id(&dataset.primary_id);
    }

    assert_events_match_exact(
        history_repo.history().await,
        &dataset.events,
        "전체 이력은 순번 순서와 모든 사건 필드를 그대로 보존해야 함",
    );

    let expected_primary: Vec<_> = dataset
        .events
        .iter()
        .filter(|event| event.instrument_id() == &dataset.primary_id)
        .cloned()
        .collect();
    let expected_secondary: Vec<_> = dataset
        .events
        .iter()
        .filter(|event| event.instrument_id() == &dataset.secondary_id)
        .cloned()
        .collect();
    assert_events_match_exact(
        history_repo.history_for_id(&dataset.primary_id).await,
        &expected_primary,
        "primary_id 이력은 해당 종목 사건만 순번 순으로 반환해야 함",
    );
    assert_events_match_exact(
        history_repo.history_for_id(&dataset.secondary_id).await,
        &expected_secondary,
        "secondary_id 이력은 해당 종목 사건만 순번 순으로 반환해야 함",
    );
    assert_eq!(
        history_repo.history_for_id(&dataset.quiet_id).await,
        Ok(Vec::new()),
        "사건이 없는 등록 종목의 이력은 빈 벡터여야 함"
    );
    assert_eq!(
        history_repo.history_for_id(&dataset.unknown_id).await,
        Ok(Vec::new()),
        "미등록 종목의 이력은 빈 벡터여야 함"
    );
}
