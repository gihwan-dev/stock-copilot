# D-45 거래내역 동기화 창과 일일 전체 검사

- 상태: 초안
- 기준: afc9a3f9841832585bcc48fe122e4f36cd27b367
- 근거: I-06, ADR 0034, ADR 0090, A-07 ADR의 정의 대기

## 결정

최근 14일은 KST 오늘을 포함한 연속 14개 결제일이다. 시작은 오늘보다 13일 전이며 양끝을 포함한다. ADR 0034의 닫힌 창을 따르되 오늘 포함 여부는 기존 문서에 없어 여기서 정한다. 영업일이 아닌 달력 날짜다. 입력 시각의 보존 오프셋과 무관하게 +09:00으로 날짜를 계산한다. domain은 시계를 읽지 않는다.

`recent_transaction_window(now)`는 기존 봉인 타입 `SettlementWindow`를 반환하므로 A-07의 명시적 창과 직접 호환된다. 날짜 계산은 시각에서 312시간을 빼는 방식이 아니라 달력에서 13일을 뺀다. 1년 1월 14일 KST 자정도 올바르게 지원한다. 표현 범위 1..9999년 밖은 축소·포화하지 않고 OutOfRange로 거부한다.

`daily_full_inspection(last_completed, now)`는 마지막 성공한 전체 검사 시각이 없거나 KST 날짜가 다르면 Required, 같으면 CompletedToday다. 두 시각의 KST 날짜 범위 오류를 먼저 거부한다. 표현 가능한 마지막 시각이 현재보다 미래면 나노초 단위로 ClockReversed다. 이 함수는 실제 전체 검사 완결이나 저장 사실을 증명하지 않는다. 마지막 완료 시각은 전체 기간의 모든 구간이 성공한 뒤에만 호출자가 저장해야 한다.

## 공개 계약

| 타입·함수 | 계약 |
|---|---|
| SettlementWindow | 기존 비공개 양끝 날짜, 닫힌 창 |
| recent_transaction_window(Timestamp) | Result<SettlementWindow, TransactionSyncTimeError> |
| DailyFullInspection | Required / CompletedToday |
| daily_full_inspection(Option<Timestamp>, Timestamp) | Result<DailyFullInspection, TransactionSyncTimeError> |
| TransactionSyncTimeError | OutOfRange / ClockReversed |

## 인계

| 받는 작업 | 필요한 일 |
|---|---|
| A-22 | Clock 값을 전달하고 계좌별 마지막 성공 전체 검사 시각을 저장. 실패·부분 수신은 완료 시각 갱신 금지. 전체 시작일·상한 초과 구간 분할과 완결 저장·재시도 계약 연결 |
| S-03 | 전체 검사 완료 시각을 계좌별로 원자적으로 보존 |

## 검증

KST 자정 전·정각·후 나노초, UTC와 다른 날짜, 월·연 경계, 윤일, 양끝과 창 밖, 최소·최대 표현 범위, 오프셋 표현 불변성을 예제·독립 달력 오라클 속성 테스트로 검사한다. 새 외부 의존성이나 기존 테스트 변경은 없다. 세션 프로젝트 신뢰와 훅 신뢰는 확인할 수 없으며 정상 Git 훅으로 검사한다.

## 손 변형

최종 구조의 임시 복사본에서 26개 변형을 실제 nextest로 실행했다. 26개 모두 컴파일 성공·검출이며 생존은 없다. 앞 구조의 실험과 섞어 세지 않는다.

| 번호 | 바꾼 것 | 검출 테스트 |
|---|---|---|
| 1 | 시작일 감산 13일 → 0일 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 2 | 시작일 감산 13일 → 1일 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 3 | 시작일 감산 13일 → 7일 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 4 | 시작일 감산 13일 → 12일 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 5 | 시작일 감산 13일 → 14일 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day |
| 6 | 시작일 감산 13일 → 15일 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day |
| 7 | 시작일 감산 13일 → 28일 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day |
| 8 | KST 오프셋 32400 → 0 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 9 | KST 오프셋 32400 → 28800 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 10 | KST 오프셋 32400 → 36000 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 11 | KST 오프셋 32400 → -32400 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 12 | KST 오프셋 32400 → 32340 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 13 | KST 오프셋 32400 → 32460 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 14 | 역행 동률 거부 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond |
| 15 | 역행 검사 반전 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance |
| 16 | 역행 검사 제거 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance |
| 17 | 완료일 비교 반전 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance |
| 18 | 마지막 날짜를 현재로 대체 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance, unrepresentable_windows_and_kst_dates_fail_closed |
| 19 | 미래 오류를 범위 오류로 변경 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance |
| 20 | 기록 없음 완료 처리 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond |
| 21 | 동일 날짜 재검사 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance |
| 22 | 다른 날짜 완료 처리 | daily_inspection_uses_calendar_day_and_rejects_future_even_one_nanosecond, independent_calendar_oracle_and_offset_invariance |
| 23 | 최소 연도 전체 거부 | kst_midnight_before_exact_after_and_utc_day |
| 24 | 연도 범위 반전 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 25 | 날짜 감산을 가산으로 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day, unrepresentable_windows_and_kst_dates_fail_closed |
| 26 | 창 양끝 뒤바꿈 | independent_calendar_oracle_and_offset_invariance, kst_midnight_before_exact_after_and_utc_day |

앞 구조의 변경분 변이는 7개 중 검출 4개, 컴파일 불가 3개, 생존 0개, 시간 초과 0개였다. 최초 실행은 dist 부재, 두 번째는 비표준 테스트 경로의 test-diff 도달성 오류로 실패했다. 최종 구조 재실행은 11개 중 검출 7개, 컴파일 불가 4개, 생존 0개, 시간 초과 0개로 종료 0이다. 검사 규칙과 기존 테스트는 바꾸지 않았다.

날짜 뺄셈은 CalendarDate 내부에서 수행한다. time Date의 하한은 domain의 1년보다 훨씬 이르므로 1..9999년 날짜에서 13일을 빼면 라이브러리 하한에 도달하지 않는다. 내부 saturating_sub가 포화하는 입력은 봉인 타입으로 만들 수 없고, 결과가 1년 미만이면 오류로 반환한다. 기존 SettlementWindow 모듈에서 비공개 필드로 창을 만들어 불가능한 역순 오류 경로를 추가하지 않는다. 테스트는 측정 정책에 맞춰 cfg(test) 인라인 모듈에 둔다.


중간의 이전 공개 경로가 섞인 변이 실행은 유효 검출 집계에서 제외한다.

속성 테스트의 시각 생성 범위는 모든 허용 오프셋 표현에서 Timestamp를 만들 수 있도록 잡는다. 최댓값·KST 변환 범위 밖은 별도 고정 예제로 검증한다.
