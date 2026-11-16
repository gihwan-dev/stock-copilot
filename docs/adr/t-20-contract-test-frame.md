# T-20 계약 테스트 틀

- 상태: 초안

## 결정

실제 외부 호출 없이 손으로 만든 합성 JSON을 `crates/adapters/broker-toss/tests/recordings/`에 둔다. harness 6.1은 경로를 지정하지 않으므로 작업 지시의 기본 경로를 쓴다. 파일은 origin(synthetic), method(get), path(OpenAPI 템플릿), status(문자열), schema(로컬 JSON Pointer), body를 갖는다. 계좌 식별자는 실제 계좌번호가 아닌 synthetic-account-731이다.

디렉터리를 자동 열거하여 모든 파일을 현재 스펙과 대조한다. `cargo nextest run --workspace --locked`가 통합 테스트 contracts를 자동 발견하므로 기존 `just check`에 별도 명령 없이 등록된다. 기존 서버 함수와 testkit은 수정하지 않는다. `serve_recordings(&recording)` 한 줄이 검증을 거쳐 녹화 하나당 루프백 서버 하나를 띄운다. 여러 녹화는 독립 서버로 재생한다. 상태별 같은 경로도 충돌하지 않고 기존 서버의 수명·타임아웃·요청 회수 방식을 그대로 쓴다. 범용 제품 전송 포트나 API 파싱은 추가하지 않는다.

테스트 도우미는 `tests/support_tests/`에 둔다. source-check의 시험 모듈 이름 규칙을 지키기 위한 이름이다. 후속 통합 테스트는 `#[cfg(test)] mod support_tests;`로 가져온다. `spec`, `recordings`, `route`, `serve_recordings`를 재사용한다. 현재 닫힌 GET 경로 전부를 대응하며 경로 인자는 synthetic-731로 고정한다. 쿼리·계좌 헤더는 후속 API 테스트가 ReadRequest에서 직접 구성한다.

T-01 생성 ApiResponse/ErrorResponse를 역직렬화하고, 이번 두 성공 응답의 result를 각각 Vec<Account>와 ExchangeRateResponse로 추가 역직렬화한다. 구체 생성 타입만 직접 호출하는 독립 테스트가 필드 타입 오류를 거부하는지도 확인한다. 성공 envelope의 result는 구체 타입이 없고, 생성 enum은 unknown을 보존하므로 이것만으로 엄격한 녹화 검사가 불가능하다. 현재 원문 스키마를 추가 대조한다. 필요한 부분만 직접 검사하며 새 외부 패키지는 추가하지 않는다. 지원하지 않는 키워드는 성공으로 넘기지 않고 오류로 돌려준다.

| 항목 | 검사 |
|---|---|
| 없는 GET 경로·상태·틀린 schema 포인터 | 실패 |
| 필수 필드 누락·중첩 필드 타입·배열 항목 타입 | 실패 |
| enum 밖 값 | 실패. 제품의 unknown 보존 동작에는 영향 없음 |
| 로컬 $ref, allOf, oneOf, nullable, 다중 type, 추가 속성 규칙, maxLength | 검사. 참조 순환은 깊이 제한으로 실패 |
| pattern, anyOf, 수치 범위 등 미지원 키워드 | 해당 스키마를 사용하는 녹화는 실패. 조용히 무시하지 않음 |
| format(decimal, date-time, int64) | 현재 형식 의미는 검사하지 않음. JSON 타입만 검사 |
| 추가 속성을 금지하지 않은 스키마의 미지 필드 | 원문대로 허용. 필드 오타를 모두 잡는 검사는 아님 |
| result와 error의 동시 존재 | 원문의 설명 문장에는 금지돼 있으나 검증 키워드에 없어 현재 검사하지 않음 |
| accountSeq의 413.0 같은 정수 값 | JSON Schema보다 생성 Rust i64와 serde_json 정수 표현 판정이 엄격하여 거부 |
| 실제 토스 서비스의 동작·금융 계산 의미·인증·페이지 연결 | 이 작업에서 증명하지 않음 |

## 사용 예

```rust
let recording = support_tests::recordings().remove(0);
let server = support_tests::serve_recordings(&recording);
let endpoint = TossEndpoint::local(server.port())?;
let client = TossHttpClient::new(&endpoint)?;
```

위반 고정 자료는 시험의 정확한 JSON 값이다. 필수 필드 추가·타입 변경을 메모리의 스펙 복사본에 적용하여 기존 녹화가 실패하는지 검사한다. 원문 스펙 파일은 수정하지 않는다. 계좌 순번·환율·기준율·기준점·시각 값은 서로 다르게 둔다.

프로젝트 신뢰 및 현재 세션의 훅 해시 신뢰와 런타임 적용은 확인 불가다. 저장소 설정에 hooks=true가 있는 사실과 실제 적용은 구분한다. pnpm frozen 설치와 lefthook install은 성공했다. Git 훅 및 최종 검사를 검증 근거로 쓴다.

## 인계

- T-05 이후: 새 성공 경로의 녹화를 추가할 때 typed.rs의 구체 생성 타입 분기와 그 타입의 잘못된 입력을 거부하는 독립 테스트도 추가한다. 현재 두 경로 외에는 생성 envelope만 확인하지만 원문 스키마 대조는 계속 적용된다. 녹화 파일을 추가하면 공통 검증·재생에 자동 포함된다. 제품 DTO 파싱 및 포트 테스트는 각 작업이 추가한다. 미지원 스키마 키워드를 쓰는 엔드포인트는 그 검증을 추가해야 한다.
- T-03: master에 시계·대기 주입 서명이 들어오면 테스트 클라이언트 생성만 맞춘다.
- T-14: 실제 응답 비식별화는 별도 작업이다. 이 자료는 합성 자료다.

## 손 변형

`mktemp -d`로 만든 독립 복사본에서 각 소스를 바꾸고 `cargo nextest run --locked -p broker-toss --test contracts`를 실행했다. 아래 28개는 모두 컴파일 성공 후 테스트 실패로 검출됐다. 상세 로그는 target/T-20/mutation-NN.log에 있다. 구체 생성 타입을 제거한 최초 두 시도는 미사용 import 때문에 컴파일되지 않아 세지 않았다. 모델 타입을 유지하면서 null을 검증하게 우회한 두 변형으로 교체해 검출을 확인했다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | concrete Account omitted | concrete_generated_types_validate_success_payloads_independently, concrete_generated_types_validate_success_payloads_independently |
| 2 | concrete ExchangeRate omitted | concrete_generated_types_validate_success_payloads_independently, concrete_generated_types_validate_success_payloads_independently |
| 3 | nullable conjunction | one_of_requires_exactly_one_matching_branch, one_of_requires_exactly_one_matching_branch |
| 4 | oneOf count | one_of_requires_exactly_one_matching_branch, one_of_requires_exactly_one_matching_branch |
| 5 | origin comparison | recordings_replay_over_local_client, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types |
| 6 | method comparison | all_recordings_match_current_spec_and_generated_types, invalid_recordings_are_rejected, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, invalid_recordings_are_rejected, recordings_replay_over_local_client |
| 7 | metadata conjunction | invalid_recordings_are_rejected, invalid_recordings_are_rejected |
| 8 | schema comparison | recordings_replay_over_local_client, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types |
| 9 | envelope direction | concrete_generated_types_validate_success_payloads_independently, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, concrete_generated_types_validate_success_payloads_independently, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client |
| 10 | depth boundary | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 11 | depth off by one | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 12 | type acceptance | one_of_requires_exactly_one_matching_branch, schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, one_of_requires_exactly_one_matching_branch, schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 13 | enum acceptance | schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, invalid_recordings_are_rejected, schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, invalid_recordings_are_rejected |
| 14 | required presence | schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client |
| 15 | allOf ignored | schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected, schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected |
| 16 | properties ignored | schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected, schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected |
| 17 | items ignored | schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected, schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected |
| 18 | maxLength boundary | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 19 | maxLength comparison | schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client |
| 20 | object type | schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 21 | array type | schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 22 | string type | schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 23 | integer type | schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, one_of_requires_exactly_one_matching_branch, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, one_of_requires_exactly_one_matching_branch, all_recordings_match_current_spec_and_generated_types |
| 24 | number type | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 25 | boolean type | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 26 | null type | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 27 | union intersection | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 28 | extra field accepted | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |

## 보호 파일 등록

`harness/arch-check/rules.toml`의 broker-toss http-write 허용 파일에 이 작업의 계약 시험 파일과 검증 파일만 추가한다. 녹화 머리 정보의 method 문자열과 잘못된 post 녹화 거부 시험이 범용 HTTP 메서드 표식에 걸리기 때문이다. 제품 코드의 쓰기 전송 허용 범위는 넓히지 않는다. 작업 절의 새 파일 한정 등록 승인에 따른다. 설정·잠금 조각·훅 해시는 공식 생성기로만 재생성한다.

## 검사

- `cargo nextest run --locked -p broker-toss --test contracts`: 5개 통과.
- `cargo clippy --locked -p broker-toss --all-targets -- -D warnings`: 통과.
- `just mutants-diff <최초 기준>`: 최초 시도는 frontendDist 미생성으로 테스트 목록 생성 실패. `just frontend-build` 후 재실행은 위반 없음. 변경된 자동 변이 대상 제품 src가 없어 선택 변이 0개다. 자동 변이 검출 성공 수로 세지 않는다.
- 기존 테스트 변경 및 승인 test-diff 예외: 0건. 기존 impl, 서버 함수, 의존성 변경 없음.

`check-fast`의 전체 승격 첫 실행은 새 validate 함수의 60줄 초과로 clippy에서 실패했다. 길이 검증을 별도 함수로 분리했고 좁은 clippy와 계약5개는 통과했다. 정상 커밋 훅에서 전체를 다시 검사한다.

리뷰 수정: 성공 본문의 구체 생성 타입 재사용을 보강했다. 생성된 환율 타입도 decimal/date-time의 의미는 검사하지 않으며, enum은 unknown을 보존한다. 따라서 원문 enum 대조와 별도의 형식 의미 인계를 유지한다. 쿼리 파라미터 및 봉투 상호 배제의 서술상 계약은 T-05 이후 각 엔드포인트 테스트에 인계한다.

## 재리뷰 후 확정한 인계

- T-05 이후: 녹화 최상위의 headers·query 등 추가 키는 현재 검증·재생에 반영되지 않는다. 요청 쿼리와 헤더는 ReadRequest에서 구성한다. 미지원 키를 적는 것만으로 동작을 추가할 수 없다.
- T-05 이후: schema 선언은 포인터 문자열 자체가 아니라 가리키는 JSON 값의 동등성을 검사한다. 같은 스키마를 가리키는 다른 상태의 포인터 표기도 통과할 수 있다. 실제 본문 검증은 path·status가 선택한 스키마로 하므로 검증 스키마를 바꾸지는 못한다.
- T-05 이후: 신규 두 테스트 파일의 http-write-means는 파일 단위로 제외돼 그 규칙의 향후 쓰기 수단 탐지가 줄어든다. 다른 HTTP·주문 안전 검사는 유지된다. 실제 쓰기 전송을 이 파일에 추가해서는 안 되며, 메타데이터와 부정 시험의 표식 때문에 승인된 등록이라는 이유를 유지한다.

구체 생성 타입 보강 후 계약 테스트는 6개다. 재리뷰의 원래 7개라는 표현은 독립 비판에서 소스와 탐침을 구분해 6개로 바로잡았다. 두 차례 새 Opus 리뷰와 각각 새 Codex 비판을 거쳐 차단·중요는 0건이며 사소 3건은 위 인계로 남긴다.
