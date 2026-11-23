# D-56: 제안 초안과 신호의 복원 근거

- 상태: 초안

## 결정

과거 추정 과제는 당시 종목 마스터 행, 가격 관측 시각과 가격, 일봉, 투자 기간·고급 시간대 허용 상태·위험 성향을 근거로 남긴다. 복원은 봉에서 변동성을 다시 계산하고 `EstimateTask::create`를 다시 호출한다. 원래 판정의 표시값을 별도로 보존해 재계산 결과가 다르면 거부한다. 저장된 마스터 행의 식별자·시장·심볼·통화가 별도로 받은 현재 마스터 조회와 맞아야 하며, 주문 종목과 시세 관측은 현재 조회에서 다시 만든다. 마스터의 시각과 제안 만료 시각은 현재 시각과 비교하지 않는다.

현재 종목 마스터의 거래 가능성 검사는 실거래 경로와 과거 이력 경로로 나눈다.
- 실거래 경로(`EstimateRestoreEvidence::capture`, `DraftRestoreEvidence::restore`, `SignalRestoreEvidence::capture`)는 현재 마스터의 주문 게이트(`order_gate_facts`)를 통과해야 한다. 거래정지·상장폐지·상장예정·정리매매 매수·미지 증권 유형·한국 거래 상태 누락 종목은 새 초안이나 실거래 제안으로 복원하지 않는다.
- 과거 이력 경로(`StoredEstimateEvidence::restore_history`, `StoredSignalEvidence::restore_history`, `StoredSignalOccurrence::restore`, `HistoricalPortfolio::from_saved_facts`)는 저장 당시 마스터 행의 거래 가능성과 현재 마스터의 식별(`instrument_id`, `market`, `symbol`, `currency`) 일치만 확인한다. 이미 저장된 신호가 나중에 거래정지·상장폐지 등으로 바뀌어도 조회·폐기·무효화·재평가로 정리할 수 있다.

제안은 추정 근거, 카드 시세, 계좌·주문 조건, 생성 시각·유효 기간·만료 시각을 보존한다. 원래 범위와 만료 시각을 기존 생성자로 재생성해 비교한다. 초안의 주문 조건은 `DraftOrderIntent`와 `HistoricalOrderTerms`로 읽으며 저장된 수량·금액은 과거 값 타입이다. 검증된 제안의 주문 조건과 실행 토큰은 기존 경로를 유지한다. 과거 값 타입은 주문 수량 타입이나 주문 전송 타입(`OrderWireValue`)으로 직접 변환하지 않고 `HistoricalDecimalText`로만 표시·저장한다. 수치 표시는 저장소가 기록할 수 있도록 제공하지만 이는 암호학적 변조 방지나 임의의 프로그램이 값을 다시 파싱하는 것을 막는 수단은 아니다.

신호 근거는 비용 원자료·비용 기간, 보정 스냅숏과 원장 위치, 제출 확률과 출처, 당시 평가 문맥을 보존한다. 기존 비용 적격, 보정, 임계값, 출처·정책·시간 판정을 다시 실행해 당시 표시·방향과 비교한다. `HistoricalSignal`과 `HistoricalSignalOccurrence`는 화면·이력용이며 `BuySignal`이나 현재 수량 산정 입력으로 직접 변환하지 않는다. `display_facts`는 당시 매수 한도와 매도 수량까지 제공하되 과거 값 타입으로 감싼다. 발생 이력은 원래 매수 신호와 달력을 다시 사용해 구간을 계산하고 저장된 구간과 대조한다.

제안 포트의 저장은 `Proposal<Draft>`, 복원 근거와 현재 마스터 조회를 함께 받는다. 조회는 제안 ID와 현재 마스터 조회를 받는다. 메모리 가짜도 근거로 복원한 초안과 저장 요청을 대조하고, 현재 식별이 다르거나 거래 불가로 바뀌면 실패한다.

신호 평가 포트의 원자적 커밋은 발생 신호가 있을 때 `SignalCommitEvidence`를 함께 받는다. 현재 신호와 이력 조회는 각각 `HistoricalSignal`, `HistoricalSignalOccurrence`를 반환한다. 평가 입력에는 과제·수수료·세율·만기·원래 포트폴리오·환율과 보유 종목 마스터 목록(`holding_masters`)의 복원 근거가 필요하다. 근거가 없거나 마스터가 충돌하거나 실제 평가와 맞지 않으면 신호 커밋과 이벤트를 내지 않는다. 메모리 가짜는 대상 종목 마스터의 식별자 일치(`evidence.master`↔`evidence.signal`)와 발생 이력 근거의 신호 근거·오프셋 결합(`StoredSignalOccurrence::verify_commit_binding`)을 확인하고, 대상 종목과 보유 종목 마스터를 검증·병합해 커밋 근거를 원자적으로 저장하며(이미 등록된 현재 마스터를 덮어쓰지 않고 신규 신호 커밋 시 현재 저장소 마스터의 주문 게이트까지 확인한다), 조회 때마다 저장된 근거와 현재 마스터 목록으로 `HistoricalSignal`·`HistoricalSignalOccurrence`를 다시 복원한다.

두 포트가 현재 마스터를 받는 방식은 의도적으로 다르다. `ProposalRepository`는 단일 종목의 실거래 `Proposal<Draft>`를 반환하므로 호출자가 확보한 단일 현재 마스터(`&MasterInstrument`)를 메서드 인자로 받아 주문 게이트까지 검사한다. 반면 `SignalRepository`는 A-27에서 확정된 조회 계약(`snapshot`, `current`, `occurrences`)을 유지하면서 단일 대상 종목뿐 아니라 당시 포트폴리오에 포함된 여러 보유 종목의 마스터까지 함께 검증해야 하므로, 저장소 어댑터가 `SignalCommitEvidence`의 마스터와 최신 마스터 저장소를 조회해 복원한다.

평가 당시 보유·현금은 `HistoricalPortfolioFacts`로 읽으며 현재 `Portfolio`를 내보내지 않는다. 과거 현금(`HistoricalBuyingPower`)은 `Money`를 직접 노출하지 않고 `HistoricalBuyingPowerState`(`Available(HistoricalCashMoney)`, `NegativeBuyingPower`, `CashLookupFailed`)로만 표현해 외부에서 `BuyingPower::new`로 재조립할 수 없게 한다. 보유 수량 역시 과거 수량 타입으로만 노출해 주문 수량 생성자에 직접 넘길 수 없다. 각 과거 보유 종목(`HistoricalHolding`)은 `instrument_id`를 함께 보존하며, 저장된 보유 사실을 다시 조립할 때 `instrument_id`로 유일한 현재 마스터를 찾은 뒤 심볼·시장·통화 일치를 대조해 심볼 재사용으로 다른 종목이 매칭되는 것을 막는다. 상장폐지된 원래 마스터와 심볼을 재사용한 새 활성 마스터가 마스터 목록에 공존하더라도 `from_saved_facts`는 `instrument_id` 매칭·`(symbol, market, currency)` 일치와 정준 정렬만 검증하므로 기존 신호의 조회·폐기·무효화·재평가가 막히지 않는다. 비폐지(`Scheduled`·`Active`) 마스터 간 심볼 중복 금지는 `from_saved_facts` 읽기 경로가 아니라 마스터 저장·병합 경계(`merge_evidence_masters` 및 S-22 마스터 저장소 제약)에서 강제한다. 분류·증권 유형도 `HistoricalClassifications`로 보존하고 내부 재판정에만 사용한다. 과거 유효한 값과 표시 결과를 별도 원본 필드와 대조해 단일 필드의 손상을 검출한다. 이 중복은 오류 검출을 위한 것이며 두 사본을 함께 바꾸는 변조에 대한 인증은 제공하지 않는다.

같은 UTC 순간이어도 추정 과제와 신호의 원본·저장 근거, 만기, 환율·시장 시간, 보유·현금 관측과 매도 표시의 오프셋이 바뀌면 저장 또는 복원 오류다. 복원된 과거 계좌·현금·보유 수량과 매수 한도는 `Debug` 출력에서 가린다.

## 확인 중

- 3회차 적대적 리뷰·독립 비판과 최종 수정(`119137a7`), `origin/master`(`0b223855`) `--no-ff` 병합(`9b60ce3d`), `domain` 줄·영역 커버리지 100%(`coverage-check: 위반 없음`), `check-fast-static`, `disable-report-since`(새 끄기 0건), `test-diff`(승인 29건 정확 일치), 최종 손 변형 27/27(누적 75/75) 검출을 완료했다. Docker 부재로 원격 CI에서 전체 검사를 최종 확인한다.

## 인계

- E-01·A-12·S-18: 추정 과제를 발급·저장하고 신호 평가 입력을 조립하는 측이 `StoredEstimateEvidence`(당시 마스터 행, 일봉, 기준 시세, 과제 시각·기간·허용 상태·성향)와 비용·만기 근거(`commission`, `tax`, `maturity`) 및 보유 종목별 `holding_masters`를 함께 보존·공급해야 한다.
- A-14: 제안 초안을 생성·저장할 때 추정 근거(`StoredEstimateEvidence`), 카드 시세, 유효 기간으로 `DraftRestoreEvidence`를 조립해 `ProposalRepository::store_draft`에 넘겨야 한다.
- S-22: SQL·JSON 형식과 원자적 저장, 현재 마스터 조회를 주입하는 제안·신호 및 발생 이력의 SQLite 연결은 별도다. `HistoricalHolding::instrument_id` 기준 보유별 현재 마스터 조회와 누락·중복·식별 불일치의 닫힌 실패, 상장폐지 심볼 재사용 공존 허용과 비폐지(`Scheduled`·`Active`) 마스터 심볼 유일성 제약, 커밋 시 `evidence.master`↔`evidence.signal` 식별자 일치 및 `StoredSignalOccurrence::verify_commit_binding`(오프셋 포함) 대조, 비거래 종목의 과거 이력 조회 허용과 실거래 초안 복원·신규 신호 커밋 거부를 구현해야 한다.
- D-55: 확인된 티커 변경 이력을 도입할 때 대상 종목뿐 아니라 과거 보유 종목(`HistoricalHolding`)의 이전 심볼 이력 대조·조정 경로도 함께 정의해야 한다.
- O-04·O-08: 주문 전 제안 조회 시 현재 마스터 조회를 선행해 `ProposalRepository::draft`에 전달하며, 필요하면 두 포트의 마스터 조회 주입 방식을 저장소 어댑터 기준으로 통일하는 후속 정리를 검토할 수 있다.

## 손 변형

작업 사본의 `target/D-56/` 아래 격리 사본에서 변형을 하나씩 적용하고 해당 테스트를 실행했다. 1–32번은 복원·오프셋 보강 회차, 33–39번은 과거 수량 타입 보강 회차, 40–48번은 저장 시 원본 오프셋 대조 보강 회차에서 실행했다. 유효한 48개 모두 컴파일 후 테스트 실패로 검출됐다. 초기 생존 또는 컴파일 실패 사례는 테스트나 변형을 보완해 다시 실행했다.

| 번호 | 변형 | 검출 테스트 |
| --- | --- | --- |
| 1 | 초안 ID 비교 제거 | `individual_draft_evidence_fields_are_required` |
| 2 | 계좌 비교 제거 | `individual_draft_evidence_fields_are_required` |
| 3 | 주문 조건 비교 제거 | `individual_draft_evidence_fields_are_required` |
| 4 | 생성 시각 비교 제거 | `individual_draft_evidence_fields_are_required` |
| 5 | 유효 기간 비교 제거 | `individual_draft_evidence_fields_are_required` |
| 6 | 카드 시세 비교 제거 | `individual_draft_evidence_fields_are_required` |
| 7 | 만료 시각 비교 제거 | `proposal_replay_rejects_changed_expiry_or_range_evidence` |
| 8 | 만료 오프셋 비교 제거 | `stored_offsets_are_checked_even_when_the_instant_is_unchanged` |
| 9 | 카드 오프셋 비교 제거 | `stored_offsets_are_checked_even_when_the_instant_is_unchanged` |
| 10 | 추정 근거 원본 비교 제거 | `independently_recorded_evidence_rejects_valid_but_changed_fields` |
| 11 | 현재 종목 일치 조건 반전 | `individual_estimate_evidence_fields_are_required` |
| 12 | 봉 통화 비교 반전 | `individual_estimate_evidence_fields_are_required` |
| 13 | 추정 표시 결과 비교 반전 | `individual_estimate_evidence_fields_are_required` |
| 14 | 기준 시각 오프셋 비교 반전 | `individual_estimate_evidence_fields_are_required` |
| 15 | 과제 시각 오프셋 비교 반전 | `individual_estimate_evidence_fields_are_required` |
| 16 | 신호 원본 근거 비교 제거 | `independently_recorded_evidence_rejects_valid_but_changed_fields` |
| 17 | 제출 시각 오프셋 비교 반전 | `signal_history_replays_buy_and_sell_facts_without_exposing_live_signal` |
| 18 | 신호 표시 비교 반전 | `signal_history_replays_buy_and_sell_facts_without_exposing_live_signal` |
| 19 | 발생 ID 비교 반전 | `signal_occurrence_replays_original_interval_and_rejects_damage` |
| 20 | 발생 구간 비교 반전 | `signal_occurrence_replays_original_interval_and_rejects_damage` |
| 21 | 보유 종목 단일 마스터 조건 반전 | `stored_portfolio_facts_require_matching_unique_master_to_rebuild_history` |
| 22 | 분류 중복 검사 반전 | `restored_signal_rejects_duplicate_historical_classifications` |
| 23 | 신호 매수 한도 Debug 노출 | `historical_evidence_debug_redacts_account_and_portfolio_values` |
| 24 | 과거 현금 Debug 노출 | `historical_evidence_debug_redacts_account_and_portfolio_values` |
| 25 | 과거 계좌 Debug 노출 | `historical_evidence_debug_redacts_account_and_portfolio_values` |
| 26 | 과거 수량 Debug 노출 | `historical_evidence_debug_redacts_account_and_portfolio_values` |
| 27 | 신호 만기 오프셋 대조 제거 | `signal_history_rejects_changed_offsets_in_maturity_and_portfolio` |
| 28 | 보유 관측 오프셋 대조 제거 | `signal_history_rejects_changed_offsets_in_maturity_and_portfolio` |
| 29 | 환율 오프셋 대조 제거 | `signal_history_rejects_changed_offsets_in_calendar_and_rate` |
| 30 | 시장 시작 오프셋 대조 제거 | `signal_history_rejects_changed_offsets_in_calendar_and_rate` |
| 31 | 매도 표시 오프셋 대조 제거 | `signal_history_rejects_changed_offsets_in_maturity_and_portfolio` |
| 32 | 과거 매도 신호 Debug 노출 | `historical_evidence_debug_redacts_account_and_portfolio_values` |
| 33 | 과거 보유 수량을 현재 Quantity로 노출 | `restored_data_cannot_become_validation_execution_or_current_quantity_authority` |
| 34 | 과거 매도 표시 수량을 현재 Quantity로 노출 | `restored_data_cannot_become_validation_execution_or_current_quantity_authority` |
| 35 | 과거 수량 표시를 0으로 대체 | `historical_signal_values_preserve_display_numbers_and_currency` |
| 36 | 과거 매수 한도 표시를 0으로 대체 | `historical_signal_values_preserve_display_numbers_and_currency` |
| 37 | 과거 매수 한도 통화를 달러로 대체 | `historical_signal_values_preserve_display_numbers_and_currency` |
| 38 | 과거 매도 수량 Debug 노출 | `historical_signal_values_preserve_display_numbers_and_currency` |
| 39 | 과거 매수 한도 Debug 노출 | `historical_signal_values_preserve_display_numbers_and_currency` |
| 40 | 과제 저장 시 원본 시각 대조 제거 | `capture_rejects_original_estimate_offset_mismatch` |
| 41 | 과제 생성 시각 오프셋 대조 제거 | `capture_rejects_original_estimate_offset_mismatch` |
| 42 | 과제 기준 시각 오프셋 대조 제거 | `capture_rejects_original_estimate_offset_mismatch` |
| 43 | 수정 기준 오프셋 대조 제거 | `capture_rejects_original_adjustment_basis_offset_mismatch` |
| 44 | 신호 저장 시 매도 표시 오프셋 대조 제거 | `capture_rejects_original_signal_offset_mismatch` |
| 45 | 신호 저장 시 제출 오프셋 대조 제거 | `capture_rejects_original_signal_offset_mismatch` |
| 46 | 저장된 기준 표시 오프셋 대조 제거 | `saved_estimate_display_offsets_match_replayed_task` |
| 47 | 저장된 생성 표시 오프셋 대조 제거 | `saved_estimate_display_offsets_match_replayed_task` |
| 48 | 신호 저장 시 과제 오프셋 대조 제거 | `capture_rejects_original_signal_offset_mismatch` |
| 49 | (`F1`) 추정 근거 `check_stored_offsets`의 `original` 대조 제거 | `stored_signal_occurrence_verify_commit_binding_checks_estimate_offsets` |
| 50 | (`F2`) 추정 근거 `check_stored_offsets`의 `expected.created_at` 오프셋 대조 제거 | `stored_signal_occurrence_verify_commit_binding_checks_estimate_offsets` |
| 51 | (`F3`) `EstimateRestoreEvidence::replay`의 `LiveOrderGate`를 `HistoricalIdentity`로 약화 | `estimate_restore_rejects_delisted_reference_and_candle_currency_errors` |
| 52 | (`F4`) 추정 근거 `check_evidence_offsets`의 일봉 `collected_at` 오프셋 대조 제거 | `estimate_restore_kills_task_master_and_candle_offset_mutations` |
| 53 | (`F5`) `DraftRestoreEvidence::restore`의 추정 복원 모드를 `HistoricalIdentity`로 약화 | `historical_restore_survives_non_tradable_current_master_while_live_draft_rejects_it` |
| 54 | (`F6`) `DraftRestoreEvidence::restore`의 `card.replay_live`를 `replay_historical`로 약화 | `draft_restore_rejects_unidentified_card_on_liquidation_buy` |
| 55 | (`F7`) `HistoricalPortfolio::from_saved_facts`의 보유 시장 정준 정렬 대조 제거 | `historical_portfolio_rejects_same_symbol_unsorted_markets` |
| 56 | (`F8`) `HistoricalPortfolio::from_saved_facts`의 보유 심볼 정준 정렬 대조 제거 | `historical_portfolio_rejects_unsorted_holdings_facts` |
| 57 | (`F9`) `HistoricalHolding::capture`의 시장 일치 조건 제거 | `identical_symbol_across_kr_and_us_markets_is_rejected_for_estimate_and_holding` |
| 58 | (`F10`) `HistoricalHolding::replay`의 중복 `instrument_id` 거부 제거 | `historical_holding_identity_rejects_symbol_reuse_and_currency_mismatch` |
| 59 | (`F11`) `HistoricalHolding::replay`의 매입 통화 일치 검사 제거 | `historical_holding_rejects_acquisition_and_current_price_currency_mismatch` |
| 60 | (`F12`) `HistoricalCashMoney::try_new`의 음수 현금 거부 제거 | `historical_portfolio_covers_buying_power_variants_and_unsorted_facts` |
| 61 | (`F13`) `SignalRestoreEvidence::capture`의 `LiveOrderGate`를 `HistoricalIdentity`로 약화 | `historical_restore_survives_non_tradable_current_master_while_live_draft_rejects_it` |
| 62 | (`F14`) `StoredSignalEvidence::verify_same_as`의 `original` 대조 제거 | `stored_signal_occurrence_verify_commit_binding_checks_signal_and_record` |
| 63 | (`F15`) `StoredSignalEvidence::verify_same_as`의 `expected` 대조 제거 | `stored_signal_occurrence_verify_commit_binding_checks_signal_and_record` |
| 64 | (`F16`) `check_signal_evidence_match`의 `estimate.check_stored_offsets` 호출 무시 | `stored_signal_occurrence_verify_commit_binding_checks_estimate_offsets` |
| 65 | (`F17`) `StoredSignalOccurrence::verify_commit_binding`의 신호 대조 제거 | `stored_signal_occurrence_verify_commit_binding_checks_signal_and_record` |
| 66 | (`F18`) `StoredSignalOccurrence::verify_commit_binding`의 `raised_at` 오프셋 대조 제거 | `stored_signal_occurrence_verify_commit_binding_checks_signal_and_record` |
| 67 | (`F19`) `StoredSignalOccurrence::restore`의 구간 종료 오프셋 대조 제거 | `occurrence_restore_kills_interval_start_and_end_offset_mutations` |
| 68 | (`F20`) `EvaluateSignals::combine_masters`의 `observed_at` 오프셋 중복 비교 제거 | `master_dedup_checks_offset_and_rejects_active_symbol_collision_in_repo` |
| 69 | (`F21`) `merge_evidence_masters`의 `observed_at` 오프셋 중복 비교 제거 | `master_dedup_checks_offset_and_rejects_active_symbol_collision_in_repo` |
| 70 | (`F22`) `merge_evidence_masters`의 비폐지 충돌 검사에서 `ListingStatus::Scheduled` 제외 | `commit_rejects_scheduled_symbol_collision_and_tampered_original_or_expected` |
| 71 | (`F23`) `merge_evidence_masters`의 비폐지 충돌 검사에 `ListingStatus::Delisted` 포함 | `delisted_holding_symbol_reuse_still_allows_signal_lifecycle_and_new_active_commits` |
| 72 | (`F24`) `verify_commit_evidence`의 `evidence.master`↔`evidence.signal` 식별자 일치 검사 제거 | `commit_rejects_occurrence_signal_swap_offset_drift_and_wrong_master` |
| 73 | (`F25`) `verify_commit_evidence`의 현재 저장소 마스터 `capture(LiveOrderGate)` 검사 제거 | `stale_tradable_commit_is_rejected_when_stored_master_is_non_tradable` |
| 74 | (`F26`) `verify_commit_evidence`의 `rebuilt.restore_history` 검증 제거 | `sell_commit_without_occurrence_rejects_tampered_original_or_expected` |
| 75 | (`F27`) `verify_commit_evidence`의 `verify_commit_binding` 검증 제거 | `commit_rejects_occurrence_signal_swap_offset_drift_and_wrong_master` |

49–75번(`F1`–`F27`)은 3차 최종 수정 커밋 `119137a7517e44ba31e9de5142a4ad9b6f0ba7d9`을 격리 복사본으로 추출해 하나씩 적용·실행한 결과이며, 27개 모두 컴파일 후 테스트 실패로 검출됐고 격리 복사본은 종료 시 삭제했다.
