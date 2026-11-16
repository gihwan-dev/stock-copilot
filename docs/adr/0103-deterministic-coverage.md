# ADR 0103: D-43 결정적 커버리지와 InvestorFlow 날짜 계약

- 상태: 확정 (2026-11-17)


## 결정

메인의 설계 변경을 따른다. 1회 탐색에서 40개 파일에 345건의 미측정 줄·영역이 나왔으므로 모든 속성 테스트 분기를 예제로 대체하는 범위는 버린다. 커버리지의 목적은 같은 코드에서 난수 때문에 검사가 간헐적으로 실패하지 않는 것이다. `just coverage-check`에만 `PROPTEST_RNG_SEED=5`을 고정한다. 훅과 CI의 전체 검사도 이 레시피를 호출한다. 일반 nextest 실행에는 시드를 설정하지 않으며 결함 탐색을 유지한다. testkit의 미설정 256회·설정값 무관 4096회와 최소 횟수 규칙은 변경하지 않는다.

proptest 1.12.0의 Config::with_cases는 cases만 지정하고 나머지는 Config::default에서 받는다. default는 PROPTEST_RNG_SEED를 읽는다. 원래 명시적으로 고정된 volatility_reference의 두 시드는 그대로다. 새 seed 설정을 testkit에 추가할 필요가 없다. 횟수를 낮춘 이전 임시 측정 패치는 모두 복구했다.

관찰된 portfolio_valuation의 ZeroCost 검증에는 수량 0의 고정 예제를 추가한다. tick_size의 두 기존 속성 테스트는 생성 전략과 단언을 보존한 채 입력별 검증을 공유 함수로 추출하고, 배수·비배수와 서로 다른 스케일의 고정 예제를 추가한다. 기존 테스트 본문 추출 두 건은 메인이 승인한 test-diff 예외다. 테스트 함수 밖으로 이동한 산술에는 하네스의 checked 연산 규칙이 적용되므로 배수·나머지·경계 계산을 checked 연산으로 표현한다. 순서 단언 뒤의 두 차는 abs_diff로, 정확한 나머지 0 단언은 checked_rem == Some(0)으로 보존한다. 양의 단위와 고정 생성 범위에서 기존 검증과 같은 조건이며 단언 삭제·범위 완화·생성 전략 변경은 없다.

InvestorFlow의 국내 기준일은 updated의 같은 순간을 KST(+09:00)로 투영한 날짜와 비교한다. 수집 시각으로 대체하지 않는다. 시각과 오프셋을 재작성하지 않고 MarketObservation에 원본을 보존한다. 투영 날짜가 표현 범위를 벗어나면 InvalidFlow로 거부한다. D-44의 SupplyContext 규칙과 같다.

## 검증

최신 master 수신 시 정상 병합 훅을 통과했다. 작업 시작 기준은 73594a8300562da73859b1f2753494d6ee8e2142다. 이후 master 730e4069f9327da8e7884c845069130d542066f7을 정상 훅으로 병합한 4bf63096b6152403eba2f34896f61edc65987c93에서 구현 변경을 복원했다. 생성물 두 파일의 충돌은 최신 master를 기준으로 기존 생성 스크립트로 재생성했다. 커버리지 대상·testkit·xtask 소스와 justfile은 측정한 stash 트리와 바이트 동일함을 대조한다. 프로젝트 신뢰·세션 훅 적용 여부는 확인할 수 없으며 Git 훅과 실행 결과를 근거로 삼는다. 홈 설정·자격 증명·실제 계좌와 API는 다루지 않았다. 새로운 의존성과 검사 끄기는 없다.

기존 측정기는 profraw를 누적해 이전 시드의 커버리지가 섞였다. 따라서 이전 탐색의 시드별 위반 수와 시드 5의 통과를 단독 증거로 사용하지 않는다. 새 회귀 테스트로 연속 측정의 계수가 늘어나는 실패를 확인한 뒤 측정 전 clean --workspace를 추가한다. profraw만 정리하면 소스 변경 전의 계측 바이너리가 남아 같은 함수의 이전 해시 버전이 0회로 보고되는 것도 실제 확인했으므로 워크스페이스 산출물을 함께 정리한다. 시드 상수는 5로 유지하되 깨끗한 프로파일에서 다시 검증한다. 단독 커버리지도 전체 검사와 같은 기존 STOCK_COPILOT_PROPTEST_CASES=4096 설정을 사용한다. 기본 일반 테스트의 256회 및 변수 설정 시 4096회 규칙과 최소 횟수 단언은 그대로다. 전체 커버리지 검사가 요구하는 희귀 분기를 기본 256회 표본의 우연에 맡기지 않기 위한 선택이며 새 횟수 정책을 만들거나 횟수를 낮추지 않는다. 고정 시드 두 번과 참고 시드 세 개는 모두 이 4096회 조건에서 측정한다. 최종 소스의 고정 시드 5 두 실행은 각각 테스트 1062개 통과·커버리지 위반 없음이며 LCOV가 바이트 단위로 같고 모든 영역의 커버 여부도 같다. LCOV SHA-256은 a1a77f7776fde2159fda926715138f481c76013d70ea51c8daff61a704d7cae3다. 참고 시드 0·1·2도 각각 테스트 1062개 통과·커버리지 위반 0건이었다. 커버리지 결과 비교는 파일별 실행 줄과 영역의 커버 여부를 사용하고 원시 계수 차이도 별도로 확인한다. 아직 실행하지 않은 결과를 성공으로 간주하지 않는다. 실제 연결한 proptest/testkit의 독립 프로세스 실험에서 미설정은 Random·256, 시드 환경 설정은 Fixed·256, 횟수 변수에 1 또는 4096 설정은 Fixed·4096임을 확인했다.

## 인계

T-10은 InvestorFlow와 D-44 수급 응답의 국내 기준일을 같은 KST 계약으로 다루고 원래 서버·수집 시각을 보존한다. 메인은 이 초안과 리뷰 기록을 평가하고 docs/tasks.md·docs/progress.md를 갱신한다. 구현자는 두 관리 문서를 직접 수정하지 않는다.

## 손 변형

mktemp -d로 만든 작업 사본 내부 복사본에서 하나씩 실제 실행했다. 무변이·복구 대조군 모두 통과. 아래 22개는 모두 컴파일 후 새 KST 회귀 테스트가 검출했으며 생존·컴파일 불가 0이다. 테스트는 서버와 수집 시각의 서로 다른 초·나노초·오프셋을 각각 비교한다.



## 독립 리뷰 후 결정

보호 파일의 필요한 등록·생성물·해시는 원 지시문의 구체적 승인으로 진행한다. 서명 옵션을 사용하지 않는 구현자 규칙을 유지한다. 번호 없는 초안도 명시된 형식이다. 첫 리뷰와 새 Codex 비판에서 동작 결함은 남지 않았으며, 확정된 회귀 공백은 테스트로 보강한다. 새 coverage_remeasurement_discards_obsolete_source_mappings는 내부 fixture의 소스 함수·줄 위치와 crate 버전을 바꾼 두 측정에서 이전 바이너리 매핑이 사라짐을 단언한다. 실제 workspace→profraw-only 손 변형이 컴파일 후 before_change 잔존 단언에서 실패하고 복구 대조군이 통과했다. 이 fixture에서는 패키지 버전을 변경해 이전 계측 바이너리가 함께 남는 조건을 만든다. 버전 변경을 제거하면 소스 이름·줄 위치 변경만으로는 profraw-only 회귀를 검출하지 못한다. 이는 이 fixture와 도구 조건에서 확인한 사실이며 바이너리 해시의 모든 입력에 대한 일반 명제는 아니다.

커버리지 통과 조건은 시드 5·4096회이며 모든 일반 속성 테스트의 모든 분기를 예제로 없앴다고 주장하지 않는다. KST 날짜 검증 두 곳은 같은 계약을 유지한다. T-10은 후속 어댑터 작업에서 둘의 의미가 갈라지지 않는지 확인한다. 현재 공통화는 완료 필수 사항이 아니므로 제품 구조는 더 늘리지 않는다.

최종 통합에서는 master 520c8dfc0e5b2fdd9bcc69a1c32b48b0eff9c49a를 수신한다. 충돌한 config/hash 생성물은 해당 master를 기준으로 기존 스크립트로 재생성하며, 사용자 관리 문서의 upstream 변경을 보존한다. 최종 HEAD 전체 검사는 `just base=HEAD check`와 그 실제 로그로 별도 확인한다.

## 최종 HEAD 검사에서 확인한 기존 fixture 충돌

master 520c8df의 ADR 0099가 커밋된 뒤 최종 HEAD 021e4c8d43687c685d9955dee50efce30607ea01에서 전체 검사를 실행하자, 기존 allowlist 테스트가 미래의 미사용 번호로 고정한 0099-future.md가 이미 사용 중인 번호가 되어 실패했다. 병합 전 훅의 HEAD에는 새 ADR 0099가 없어 이 충돌이 드러나지 않았다. 이 실패는 통과로 취급하지 않았다.

원 지시문의 준비 부분 변경 승인(d)에 따라 해당 테스트의 미사용 번호 fixture만 현재 Git 추적 ADR의 가장 큰 번호 다음 값으로 만든다. 기존 보호 경로·중복 번호 단언과 성공 기대값은 그대로 유지한다. 하네스의 허용 규칙을 바꾸지 않는다. 좁은 회귀는 수정 전 실패했고, 수정 후 allowlist 테스트 5개와 ESLint가 통과했다. 기존 테스트 변경 목록에 이 준비 부분 한 건을 별도로 기록한다. 최종 전체 검사는 수정 커밋 HEAD에서 다시 실행한다.

## 최종 손 변형 표

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | KST offset 0 | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 2 | KST offset 28800 | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 3 | KST offset 36000 | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 4 | KST offset -32400 | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 5 | KST offset 32340 | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 6 | KST offset 32460 | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 7 | date > becomes >= | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 8 | date > becomes < | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 9 | date > becomes <= | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 10 | date > becomes == | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 11 | date > becomes != | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 12 | omit future date check | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 13 | wrong date error | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 14 | projection error accepted | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 15 | stock uses collected date | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 16 | indicator uses collected date | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 17 | stock server replaced by collection | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 18 | stock collection replaced by server | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 19 | stock server offset rewritten | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 20 | indicator server replaced by collection | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 21 | indicator collection replaced by server | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 22 | indicator server offset rewritten | `deterministic_flow_dates_use_kst_and_preserve_original_observations` | 검출 |
| 23 | 계측 정리를 workspace에서 profraw-only로 축소 | coverage_remeasurement_discards_obsolete_source_mappings | 컴파일 성공·before_change 잔존 단언으로 검출, 복구 통과 |
