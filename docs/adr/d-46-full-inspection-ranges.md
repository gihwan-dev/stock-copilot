# D-46 거래내역 전체 기간 검사 구간

- 상태: 초안

## 근거

I-06, ADR 0034·0105·0110·0111은 전체 기간 검사를 요구하지만 시작 날짜를 정하지 않는다. domain은 시계나 계좌 저장소를 읽지 않는다. application은 이번에 변경하지 않는다.

## 결정

호출자가 넘긴 계좌 개설일을 시작일로 사용한다. 모르면 domain이 표현할 수 있는 가장 이른 KST 날짜인 0001-01-01부터 검사한다. 앱 연결일이나 첫 저장 행으로 축소하면 과거 누락을 숨기므로 사용하지 않는다. 사용자의 설정·절차를 추가하지 않는다. 알려진 시작일이 검사 시작 시각의 KST 오늘보다 미래면 InvalidPeriod다. 범위 밖 시각은 OutOfRange다.

검사 시작 때 전달한 시각의 KST 오늘을 끝으로 고정한다. 자정을 지나 실행해도 이 계획의 끝은 바뀌지 않는다. 시작과 끝을 포함하는 SettlementWindow로 분할한다. 상한은 양수 달력 일수이며 인자로 전달한다. 마지막 구간만 짧을 수 있다. 0은 오류이며 1과 u32 최댓값도 처리한다. 날짜의 최솟값을 만들 때 그레고리력 0001-01-01의 율리우스일 1721426을 사용한다. CalendarDate 내부에서만 날짜 산술을 수행한다.

계획은 반드시 하나 이상의 구간을 가진다. 각 구간의 정확한 양끝을 키로 결과를 수집한다. 미보고 또는 실패가 하나라도 있으면 Incomplete, 전부 성공한 경우에만 Complete다. 보고 순서는 무관하다. 같은 구간의 두 번째 보고는 결과가 같아도 DuplicateRange로 영구 Invalid가 된다. 계획 밖 구간도 UnexpectedRange로 영구 Invalid다. 반환 오류를 무시해도 이후 상태는 완료가 될 수 없다. 실패 구간 재시도는 새 계획 실행으로 한다. 진행 수집기는 Clone을 제공하지 않는다. 계획은 재사용할 수 있지만 실행마다 begin으로 새 수집기를 만든다.

성공 보고는 호출자의 증언이며 네트워크 수신·저장 성공을 domain이 직접 증명하지 않는다. status의 Complete는 구간 성공 판정이며 완료 시각을 저장할 근거로 직접 사용하지 않는다. completion_at(now)는 모든 구간 성공과 계획 끝 날짜 = 완료 표본 시각의 KST 날짜를 모두 확인한 뒤 비공개 필드의 FullInspectionCompletion을 반환한다. 이 값의 window와 completed_at만 읽을 수 있다. 자정을 넘겨 끝 날짜가 다르면 DayMismatch로 완료 기록을 거부한다. 새 오늘까지 포함한 계획으로 다시 검사하고 이 계약을 통과해야 한다. 자정 전에 만든 완료 값을 나중에 저장하더라도 completed_at을 그대로 저장하므로 다음 날을 완료로 오인하지 않는다. domain은 시계를 읽지 않으며 caller가 같은 표본을 전달한다. 분할만으로 T-06의 행·페이지 상한 아래에 든다고 보장하지 않는다.

## 공개 계약

| 타입·함수 | 의미 |
|---|---|
| InspectionRangeDays::try_new / as_u32 | 양수 구간 길이 상한 |
| FullInspectionPlan::new(opened_on, now, limit) | 시작 정책과 KST 끝을 적용한 순수 계획 |
| split_full_inspection(period, limit) | 기존 닫힌 창의 정확한 분할 |
| FullInspectionPlan::ranges / begin | 구간 목록과 실행별 수집기 |
| FullInspectionProgress::report / status | 순서 무관 보고, 중복·외부 구간 닫힌 실패 |
| FullInspectionProgress::completion_at / FullInspectionCompletion | 완료 날짜 검증, 검사 창과 저장할 정확한 완료 표본 |
| FullInspectionCompletionError | Incomplete / Invalid(InspectionReportError) / DayMismatch / OutOfRange |
| InspectionReportError | UnexpectedRange / DuplicateRange |
| InspectionRangeResult | Succeeded / Failed |
| FullInspectionStatus | Incomplete / Complete / Invalid |
| FullInspectionError | ZeroLimit / InvalidPeriod / OutOfRange |

## 인계

| 받는 작업 | 할 일 |
|---|---|
| A-22 후속 | 계좌별 개설일과 Clock 표본, 어댑터 상한으로 계획 생성. 각 구간의 완전 수신·대조·저장 성공 뒤에만 Succeeded 보고. status의 Complete를 저장 근거로 삼지 말고 completion_at(완료 Clock 표본)의 성공값에 있는 completed_at을 그대로 저장. DayMismatch면 완료 기록 없이 새 오늘까지 포함한 새 계획을 검사. 완료 값 생성 후 저장 사이에 자정이 지나도 시각을 다시 읽어 교체하지 않음 |
| T-06 후속 | 날짜 분할에도 전체 미결제를 포함하고 정상 마지막 페이지·격리 없음·행/페이지 상한을 충족해야 함. 하루가 상한을 초과해도 완료로 보정하지 않음. 개설일 미상 장기간의 반복 전체 순회 비용과 API 지원 하한을 실측·설계 |
| S-03 | 계좌별 전체 완료 시각 CAS 저장·실패 보존 |

## 검증

예제는 월·연 경계, 윤일, 하루, 정확한 상한, 최대 날짜·상한, KST 자정 전·정각·후를 고정한다. 속성 테스트는 독립 율리우스일 오라클로 구간 양끝과 합집합을 비교하고, 미보고·실패·성공 조합과 임의 우선순위에 의한 순열 보고에서 완료 동치를 검사한다. 기존 테스트 변경·새 의존성은 없다.

## 시작일 비용의 평가

개설일 미상에서 0001-01-01을 쓰는 것은 기간을 임의로 줄여 과거 누락을 숨기지 않기 위한 정책이다. 2026-11-17까지 739937일이며 상한 31/92/365일에서 구간 수는 23869/8043/2028개다. 실제 구간 상한은 호출자가 전달하며 날짜를 API에 그대로 보내야 한다는 뜻은 아니다. T-06은 현재 서버 날짜 필터 없이 전체 페이지를 받고 창으로 걸러 전체 미결제를 확보한다. 따라서 작은 창별 호출은 전체 순회를 반복한다. 이 비용은 연결 전 A-22 후속과 T-06 후속에서 반드시 평가하고, 완전 수신한 전체 자료를 구간별 대조에 재사용하는 방식을 검토한다. 실제 API 날짜 하한·당일 완료 불가능·개설일 미상 기본 경로는 실측하지 않았고 운영 장애로 확정하지 않는다. 비용 때문에 시작일을 임의로 최근 날짜로 줄이거나 일부 성공을 완료로 처리하지 않는다.
