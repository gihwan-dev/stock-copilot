# ADR 0189: 거래내역 시작일 대체 조회, 일별 평가의 종가 대체, 휴장일 추론, 구간 축소

- 상태: 확정 (2026-11-26). v0.5.4 실사용 진단 기록에서 나온 두 실패를 고친다. ADR 0188의 후속이다.

## 관찰

| 줄 | 원인 |
|---|---|
| `toss-call GET /api/v1/transactions status=400 code=malformed` → `performance_* result=failed reason=Source(... BadRequest ...)` | 전체 기간 창(ADR 0117, 0001-01-01부터)을 그대로 `from=0001-01-01`로 보냈다. 토스가 이 날짜를 형식 오류로 거부한다 |
| `daily_valuation_snapshot result=failed reason=execution-failed:Snapshot(MissingPrice)` | 관측 스냅샷은 연장 세션(국내 NXT 시간외, 미국 데이·프리·애프터마켓) 중이면 보유 응답의 현재가가 정규장 종가와 같아야 통과한다(관측 종가 계약). 16:00 KST 이후에는 거의 항상 연장 세션이라 한 종목만 있어도 스냅샷 전체가 거부된다 |

## 결정

1. 거래내역 요청의 `from`은 창 시작일이 2000-01-01 이상일 때만 보낸다. 그보다 이르면 보내지 않고 앱이 거른다. 첫 쪽이 400이고 `from`을 보냈다면 `from` 없이 첫 쪽부터 한 번 다시 받는다. 다시 거부되면 실패로 남긴다. 일반 거래내역 동기화와 전체 점검, 수익률 계산이 모두 이 경로를 쓴다.
2. 관측 스냅샷이 가격 근거 문제(`PriceBasisDefinitionPending`, `MissingPrice`, `InvalidCandle`)로 실패하면, 같은 날(`last_valuation_date`)을 원장·원주가 일봉 종가·그 날 환율로 다시 계산해 저장한다(`ComputePerformance::recompute_day`). 스냅샷의 출처는 `Recalculated`로 남고, 진단 기록은 `job id=daily_valuation_snapshot result=executed summary=recorded-from-session-closes`다. 보유 응답의 현재가 대신 마지막 정규장 종가를 쓴다는 뜻이다. 다른 실패(저장소, 보유 없음)는 그대로 실패다.
3. 장 운영 캘린더는 기간 조회가 없다(전·당·익 영업일만 준다, OpenAPI 1.2.28). 한 번 받은 캘린더에서 이웃 영업일 사이의 날을 휴장일로 추론해 캐시한다(`MarketCalendar::implied_holidays`). 주말과 휴장일에는 호출하지 않으므로, 1년 첫 계산의 캘린더 호출이 약 732회에서 시장별 영업일 수(합계 약 500회)로 준다.
4. 수익률 계산에서 첫날이나 마지막 날을 계산하지 못하면, 그 날을 빼고 원장 기간을 줄여 다시 계산한다(최대 16번). 뺀 날은 가운데 날과 함께 `skipped`에 남고, 진단 기록에 개수와 첫 이유가 붙는다.

## 버린 대안

- `from` 형식만 바꾸기: 토스가 받는 가장 이른 날짜를 문서가 정하지 않는다. 생략이 스펙상 "하한 없음"이다.
- 관측 스냅샷 계약을 domain에서 완화: 연장 세션 가격과 종가를 섞으면 일별 수익률이 흔들린다. 종가 재계산이 같은 계좌 사실로 더 일관된다.
