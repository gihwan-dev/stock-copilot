# D-46 거래내역 전체 기간 검사 구간

- 상태: 초안

## 근거

I-06, ADR 0034·0105·0110·0111은 전체 기간 검사를 요구하지만 시작 날짜를 정하지 않는다. domain은 시계나 계좌 저장소를 읽지 않는다. application은 이번에 변경하지 않는다.

## 결정

호출자가 넘긴 계좌 개설일을 시작일로 사용한다. 모르면 domain이 표현할 수 있는 가장 이른 KST 날짜인 0001-01-01부터 검사한다. 앱 연결일이나 첫 저장 행으로 축소하면 과거 누락을 숨기므로 사용하지 않는다. 사용자의 설정·절차를 추가하지 않는다. 알려진 시작일이 검사 시작 시각의 KST 오늘보다 미래면 InvalidPeriod다. 범위 밖 시각은 OutOfRange다.

검사 시작 때 전달한 시각의 KST 오늘을 끝으로 고정한다. 자정을 지나 실행해도 이 계획의 끝은 바뀌지 않는다. 시작과 끝을 포함하는 SettlementWindow로 분할한다. 상한은 양수 달력 일수이며 인자로 전달한다. 마지막 구간만 짧을 수 있다. 0은 오류이며 1과 u32 최댓값도 처리한다. 날짜의 최솟값을 만들 때 그레고리력 0001-01-01의 율리우스일 1721426을 사용한다. CalendarDate 내부에서만 날짜 산술을 수행한다.

계획은 반드시 하나 이상의 구간을 가진다. 각 구간의 정확한 양끝을 키로 결과를 수집한다. 미보고 또는 실패가 하나라도 있으면 Incomplete, 전부 성공한 경우에만 Complete다. 보고 순서는 무관하다. 같은 구간의 두 번째 보고는 결과가 같아도 DuplicateRange로 영구 Invalid가 된다. 계획 밖 구간도 UnexpectedRange로 영구 Invalid다. 반환 오류를 무시해도 이후 상태는 완료가 될 수 없다. 실패 구간 재시도는 새 계획 실행으로 한다.

성공 보고는 호출자의 증언이며 네트워크 수신·저장 성공을 domain이 직접 증명하지 않는다. Complete는 계좌 식별·완료 시각 저장 권한 토큰이 아니다. 분할만으로 T-06의 행·페이지 상한 아래에 든다고 보장하지 않는다.

## 공개 계약

| 타입·함수 | 의미 |
|---|---|
| InspectionRangeDays::try_new / as_u32 | 양수 구간 길이 상한 |
| FullInspectionPlan::new(opened_on, now, limit) | 시작 정책과 KST 끝을 적용한 순수 계획 |
| split_full_inspection(period, limit) | 기존 닫힌 창의 정확한 분할 |
| FullInspectionPlan::ranges / begin | 구간 목록과 실행별 수집기 |
| FullInspectionProgress::report / status | 순서 무관 보고, 중복·외부 구간 닫힌 실패 |
| InspectionRangeResult | Succeeded / Failed |
| FullInspectionStatus | Incomplete / Complete / Invalid |
| FullInspectionError | ZeroLimit / InvalidPeriod / OutOfRange / UnexpectedRange / DuplicateRange |

## 인계

| 받는 작업 | 할 일 |
|---|---|
| A-22 후속 | 계좌별 개설일과 Clock 표본, 어댑터 상한으로 계획 생성. 각 구간의 완전 수신·대조·저장 성공 뒤에만 Succeeded 보고. 전체 Complete 뒤에만 완료 시각 저장. 실행 중 자정 통과 시 현재 완료일 기록은 ADR0105 계약을 따르되 다음 실행에서 새 오늘까지 검사할 스케줄 정책을 평가 |
| T-06 후속 | 날짜 분할에도 전체 미결제를 포함하고 정상 마지막 페이지·격리 없음·행/페이지 상한을 충족해야 함. 하루가 상한을 초과해도 완료로 보정하지 않음. 개설일 미상 장기간의 반복 전체 순회 비용과 API 지원 하한을 실측·설계 |
| S-03 | 계좌별 전체 완료 시각 CAS 저장·실패 보존 |

## 검증

예제는 월·연 경계, 윤일, 하루, 정확한 상한, 최대 날짜·상한, KST 자정 전·정각·후를 고정한다. 속성 테스트는 독립 율리우스일 오라클로 구간 양끝과 합집합을 비교하고, 미보고·실패·성공 조합과 역순 보고에서 완료 동치를 검사한다. 기존 테스트 변경·새 의존성은 없다.
