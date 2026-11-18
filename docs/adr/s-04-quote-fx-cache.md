# S-04: 시세·환율 캐시

- 상태: 초안

## 결정

MarketDataRepository의 시세와 환율 저장은 Changed, Unchanged, OlderIgnored를 반환한다. 환율은 통화쌍과 종류마다 직전 저장 관측의 환율 값, 출처, 관측 시각이 모두 같을 때만 Unchanged다. 값, 출처, 관측 시각 중 하나라도 다르면 Changed다. 관측 시각이 직전 저장 관측보다 이르면 저장하지 않고 OlderIgnored를 반환한다. 직전보다 늦은 환율은 이전 이력을 보존한다. 같은 관측 시각에 받은 유효 종료 시각은 원문대로 갱신하지만 그 차이만으로는 환율 값·출처·관측 시각이 바뀐 것으로 보지 않는다. StoreFxRate는 저장이 성공하고 Changed일 때만 통화쌍 이벤트를 발행한다.

시세는 종목 키로 수집 시각을 비교하고, 서버 관측 시각이 양쪽에 있으면 그 순서도 비교한다. 어느 쪽이든 직전보다 이른 관측은 저장하지 않는다. 서버 시각이 없을 때는 순서를 추정해 채우지 않고 수집 시각만 대조하며 원래의 부재를 보존한다. 도메인 Quote 전체가 같으면 Unchanged, 그 밖에는 Changed다. 도메인 Quote에 없는 시장이나 가격 기준을 저장소가 추측하지 않는다. 읽기는 최신 관측을 복원한다. 현재 시각을 기준으로 한 시세의 오래됨은 판단하지 않는다.

0004_market_cache.sql은 시세, 환율 이력, 봉의 STRICT 테이블을 추가한다. 값은 기존 십진수 CHECK가 있는 TEXT와 통화로 보존한다. 환율은 통화쌍·kind·관측 시각으로 분리하고 시각의 초·나노초·오프셋과 유효 종료를 보존한다. 봉은 종목·봉 기간·수정 기준·시각으로 구분한다. 모든 읽기는 domain 생성자로 재검증한다. 저장은 Database의 단일 쓰기 트랜잭션을 사용한다.

## 인계

- A-09: 실제 시세·환율 수집과 캐시의 조립, 최신성 판단 및 UI 갱신을 연결한다.
- T-08: 실행 경로는 Changed에서만 이벤트를 내며 저장소·호출부의 실제 앱 조립은 포함하지 않는다.

## 검증

반복 환율의 이벤트 회귀는 구현 전 기존 코드에서 실패했고 구현 후 통과했다. 나중에 수집됐지만 서버 관측은 더 오래된 시세도 실제 저장소와 메모리 가짜에서 각각 실패를 재현한 뒤 고쳤다. 저장소 왕복, 다른 통화쌍·종류·종목 격리, 오래된 관측 무시, 유효 시각과 오프셋, 실패 롤백, 손상 읽기와 봉의 혼합 입력 롤백을 검사한다. 독립 리뷰 결과는 실행 후 기록한다.

## 손 변형

최종 소스와 테스트가 같은 별도 mktemp 사본과 별도 빌드 디렉터리에서 각 변형을 컴파일해 실행했다. 24개 모두 테스트가 검출했다.

| 번호 | 변형 | 잡은 테스트 |
|---|---|---|
| 1 | 시세 수집 시각 오래됨에 같은 시각 포함 | `quotes_reject_older_observations_and_preserve_instrument_isolation` |
| 2 | 시세 수집 시각 오래됨 비교 반전 | `failure_after_delete_rolls_back_the_old_quote` |
| 3 | 시세 동등 비교 반전 | `failure_after_delete_rolls_back_the_old_quote` |
| 4 | 시세 변경을 같음으로 반환 | `later_collection_does_not_replace_a_newer_server_observation` |
| 5 | 환율 관측의 오래됨에 같은 시각 포함 | `fx_roundtrip_distinguishes_kind_pair_and_atomic_outcomes` |
| 6 | 환율 관측의 오래됨 비교 반전 | `fx_roundtrip_distinguishes_kind_pair_and_atomic_outcomes` |
| 7 | 환율 관측 시각 동등 비교 반전 | `fx_roundtrip_distinguishes_kind_pair_and_atomic_outcomes` |
| 8 | 환율 값 동등 비교 반전 | `fx_roundtrip_distinguishes_kind_pair_and_atomic_outcomes` |
| 9 | USD/KRW를 반대 통화쌍으로 저장 | `corrupt_rows_fail_closed_without_affecting_other_keys` |
| 10 | 중간 환율 종류를 매수 종류로 저장 | `corrupt_rows_fail_closed_without_affecting_other_keys` |
| 11 | USD를 KRW로 저장 | `candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input` |
| 12 | 봉 주기 비교 반전 | `candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input` |
| 13 | 봉 수정 기준 비교 반전 | `candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input` |
| 14 | 봉 조회 시작 경계 제외 | `candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input` |
| 15 | 환율 시점 조회 경계 제외 | `new_fx_values_and_time_publish_events_only_after_commit` |
| 16 | 봉 고가에 시가 저장 | `candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input` |
| 17 | 봉 거래량에 종가 저장 | `candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input` |
| 18 | 봉 수집 시각에 관측 시각 사용 | `candles_roundtrip_by_instrument_period_window_and_adjustment_and_rollback_mixed_input` |
| 19 | 환율 종료 오프셋에 시작 오프셋 사용 | `fx_roundtrip_distinguishes_kind_pair_and_atomic_outcomes` |
| 20 | 알 수 없는 환율 출처 허용 | `corrupt_rows_fail_closed_without_affecting_other_keys` |
| 21 | 서버 관측 시각 역행 비교 반전 | `failure_after_delete_rolls_back_the_old_quote` |
| 22 | 서버 관측 시각 동등 경계 거부 | `quotes_reject_older_observations_and_preserve_instrument_isolation` |
| 23 | 가짜의 서버 관측 순서 비교 반전 | `later_collection_does_not_replace_a_newer_server_observation` |
| 24 | 가짜의 서버 관측 순서 검사 무력화 | `later_collection_does_not_replace_a_newer_server_observation` |
