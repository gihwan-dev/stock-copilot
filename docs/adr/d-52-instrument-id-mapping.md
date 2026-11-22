# D-52: 종목 원본 식별자와 내부 ID 대응

- 상태: 초안

## 결정

`InstrumentSource`는 알려진 국가 시장, 원문 심볼, 토스 응답의 ISIN을 함께 보존한다. `ListedStock` 및 `StockInfo`에는 `isinCode`가 있으나 내부 ID는 없다. 기존 `IsinCode`의 입력 제한은 원문 보존용이며 체크디지트 검증이나 발행 사실을 증명하지 않는다. 토스 명세는 액면분할·병합 뒤 ISIN이 달라질 수도 있다고 명시한다. 심볼이나 ISIN에서 내부 ID를 계산하지 않는다(ADR 0022).

`InstrumentMappings`는 한 저장소 원장 안에서 원본과 불투명 ID를 묶는다. `ensure`는 이미 있는 활성 원본이면 제안 ID를 무시하고 기존 ID를, 없으면 충돌 검사 후 제안 ID를 발급한다. 제안 ID는 원본의 시장·심볼·ISIN에서 유도하지 않는 독립적인 발급자가 준비한다. 빈 원장에서 선택한 제안값이 같으면 결과는 결정적이고 서로 다른 제안값으로 시작한 두 원장 사이의 ID 동등성은 약속하지 않는다. 충돌은 `IdAlreadyUsed`로 거부하며 재시도는 새로운 독립 ID로 한다. 기존에 다른 곳에서 발급된 종목 ID도 사용 중인 ID 집합에 포함한 뒤 이 계약을 사용해야 한다. 모든 원장과 저장 이력에서 ID 유일성을 보존해야 한다. 무작위 생성 자체의 충돌 확률을 유일성 보장으로 삼지 않는다. 동시 호출과 재시작 이후에도 저장소가 이 판정과 저장을 하나의 직렬화 경계에서 처리한다.

판정에는 현 시점의 목록이 충분하고 누락된 상장폐지·재상장·식별자 변경이 없음을 호출자가 확인했다는 전제가 있다. 이 코드는 외부 자료의 진위·완전성이나 관측 사이에 발생했다가 사라진 사건을 증명하지 않는다. 그 전제를 확인할 수 없으면 호출자는 원장을 조회·발급하기 전에 실패시킨다. 특히 현재 토스 응답만으로 이 전제를 충족한다고 주장하지 않는다. `retire`는 확인된 기존 원본의 종료를 기록하고 원본과 ID를 영구 보존한다. 삭제·재활성화 API는 없다. 불확정 식별 변경의 자동 이관도 없다.

| 상황                                                  | 판정                                                         |
| ----------------------------------------------------- | ------------------------------------------------------------ |
| 같은 시장·심볼·ISIN의 연속 관측, 연속성이 확인됨      | 기존 활성 ID 반환                                            |
| 같은 시장·심볼에 다른 ISIN                            | 동일 여부 불확정, `IdentityUncertain`                        |
| 같은 ISIN에 다른 심볼 또는 시장                       | 동일 여부 불확정, `IdentityUncertain`                        |
| 대소문자만 다른 심볼 또는 ISIN이 겹침                 | 원문은 보존하되 신규 발급하지 않고 `IdentityUncertain`       |
| 위의 겹침 없는 새 원본, 신규성과 연속성 전제가 충족됨 | 사용되지 않은 제안 ID로 발급                                 |
| 매핑이 없는 원본의 조회                               | `Missing`으로 실패. 심볼만으로 새 ID를 추측하지 않음         |
| 기존 원본의 확인된 상장폐지                           | `retire`가 이력을 보존하고 이후 조회·발급을 `Retired`로 거부 |
| 상장폐지 뒤 같은 값으로 재상장                        | `Retired`, 자동 복구하지 않음                                |
| 상장폐지 뒤 같은 심볼의 다른 ISIN으로 재상장          | `IdentityUncertain`, 자동 재발급하지 않음                    |
| 이미 쓰인 제안 ID로 다른 원본을 발급                  | `IdAlreadyUsed`, 상태 불변                                   |

`InstrumentMappingRepository`는 아래 연산을 제공한다. `ensure`의 확인·충돌 검사·발급·저장은 원자적이다. `retire`도 같은 직렬화 경계에 참여한다. 저장 실패나 판정 오류는 부분 변경이 없어야 한다. 가짜는 하나의 잠금으로 이 계약을 재현하며 영속 저장은 하지 않는다.

| 메서드                      | 계약                                                                         |
| --------------------------- | ---------------------------------------------------------------------------- |
| `resolve(source)`           | 정확히 같은 활성 원본의 기존 ID 또는 전용 판정 오류                          |
| `ensure(source, candidate)` | 원자적으로 기존 ID 반환 또는 제안 ID 발급. 충돌·불확실·종료는 변경 없이 오류 |
| `retire(source)`            | 기존 원본의 종료를 원자적으로 기록. 반복은 같은 ID 반환                      |

## 실측 대기

- 토스의 심볼 재사용·코드 변경·상장폐지·재상장과 ISIN 변경/재사용의 실제 표현 및 누락 없는 연속 관측을 확보하는 방법.
- `/stocks/all` 기본 상태 `ACTIVE`가 전체 상장 상태를 포함하는지. 목록의 각 심볼과 상세의 시장·ISIN·상태를 안전하게 결합하는 방법.
- 실측 전에는 원본 값의 일치나 ISIN 하나만으로 기업행위 전후의 동일성을 확정하지 않는다.

## 인계

- T-21: 종목 조회의 완전성과 상태 연속성을 먼저 입증하고 불확실하면 전용 사유로 전체 실패한다. 원본 시장·심볼·ISIN을 상세와 대조하고, 원장에 폐지 사실을 기록하는 과정과 조회·갱신 경쟁을 같은 직렬화 경계로 통합한다. 실측에서 연속성을 보장할 수 없으면 연결하지 않는다.
- S 묶음(종목 대응 저장소): 독립 ID 발급원과 원장 영속성·복원, 원본 겹침 및 ID 유일성 제약, 원자적 `ensure`·`retire`, 충돌 재시도, 장애 롤백과 동시성·재시작 계약 테스트를 구현한다. 확인된 변경을 어떻게 이관할지는 별도 결정 전까지 닫아 둔다.

## 검사

완료 기준에서 도출한 테스트를 구현 전 추가하여 미정의 API로 컴파일 실패를 확인했다. 각 소스 필드 보존, 중복 발급·제안 ID 충돌, 심볼 변경·재사용, 시장 변경, 폐지·재상장, 발급 순서 속성 및 동시에 같은 소스 발급을 검사한다. 손 변형 결과는 아래 표에 기록한다.

## 손 변형

| 번호 | 변형                                  | 결과                                                                                |
| ---- | ------------------------------------- | ----------------------------------------------------------------------------------- |
| 1    | 시장 비교 반전                        | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 2    | 시장과 심볼 조건을 OR로 변경          | 검출: `tests::same_symbol_in_different_markets_needs_a_distinct_isin_and_candidate` |
| 3    | 심볼 대소문자 비교를 구분 비교로 변경 | 검출: `tests::case_variants_never_become_distinct_new_sources`                      |
| 4    | ISIN 대소문자 비교를 구분 비교로 변경 | 검출: `tests::case_variants_never_become_distinct_new_sources`                      |
| 5    | 시장 심볼과 ISIN 결합을 AND로 변경    | 검출: `tests::uncertain_changes_fail_without_mutating_or_reusing_candidates`        |
| 6    | 불확실 분기를 구별 분기로 변경        | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 7    | 정확 조회 비교 반전                   | 검출: `tests::mapping_preserves_each_source_field_and_uses_existing_id`             |
| 8    | 활성 조회를 폐지 오류로 변경          | 검출: `tests::mapping_preserves_each_source_field_and_uses_existing_id`             |
| 9    | 폐지 조회에서 이전 ID 반환            | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 10   | 조회 불확실 조건 반전                 | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 11   | 조회 불확실 오류를 없음으로 변경      | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 12   | 조회 없음 오류를 폐지로 변경          | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 13   | 기존 대응을 새 발급으로 보고          | 검출: `tests::mapping_preserves_each_source_field_and_uses_existing_id`             |
| 14   | 기존 대응 대신 제안 ID 반환           | 검출: `tests::mapping_preserves_each_source_field_and_uses_existing_id`             |
| 15   | 판정 오류를 없음으로 변경             | 검출: `tests::case_variants_never_become_distinct_new_sources`                      |
| 16   | ID 충돌 비교 반전                     | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 17   | ID 충돌 거부 제거                     | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 18   | 저장 시장을 미국으로 고정             | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 19   | 저장 심볼을 변경                      | 검출: `tests::uncertain_changes_fail_without_mutating_or_reusing_candidates`        |
| 20   | 저장 ISIN을 변경                      | 검출: `tests::same_symbol_in_different_markets_needs_a_distinct_isin_and_candidate` |
| 21   | 저장 ID를 변경                        | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 22   | 신규 발급 결과를 기존으로 변경        | 검출: `tests::mapping_preserves_each_source_field_and_uses_existing_id`             |
| 23   | 폐지 대상 비교 반전                   | 검출: `tests::uncertain_changes_fail_without_mutating_or_reusing_candidates`        |
| 24   | 폐지 불확실 조건 반전                 | 검출: `tests::uncertain_changes_fail_without_mutating_or_reusing_candidates`        |
| 25   | 폐지 불확실 오류를 없음으로 변경      | 검출: `tests::uncertain_changes_fail_without_mutating_or_reusing_candidates`        |
| 26   | 폐지 대상 없음을 폐지 오류로 변경     | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |
| 27   | 폐지 반환 ID 변경                     | 검출: `tests::delisting_is_permanent_and_relisting_requires_external_resolution`    |

컴파일되지 않은 변형 4개는 건수에서 제외했다. 위 표는 격리 복사본에서 정상 테스트 통과를 먼저 확인한 뒤 각 변형을 따로 적용한 실제 실행 결과다. 이후 전체 훅의 커버리지 검사에서 무관한 원본의 폐지 분기가 빠진 것이 드러나 해당 회귀 테스트를 추가했다. 계측 재실행에서 새 파일의 줄·영역은 각각 100%다.
