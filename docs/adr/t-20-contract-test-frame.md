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

- T-05 이후: 녹화 파일을 추가하면 공통 검증·재생에 자동 포함된다. 제품 DTO 파싱 및 포트 테스트는 각 작업이 추가한다. 미지원 스키마 키워드를 쓰는 엔드포인트는 그 검증을 추가해야 한다.
- T-03: master에 시계·대기 주입 서명이 들어오면 테스트 클라이언트 생성만 맞춘다.
- T-14: 실제 응답 비식별화는 별도 작업이다. 이 자료는 합성 자료다.

## 손 변형

`mktemp -d`로 만든 독립 복사본에서 각 소스를 바꾸고 `cargo nextest run --locked -p broker-toss --test contracts`를 실행했다. 아래 26개는 모두 컴파일 성공 후 테스트 실패로 검출됐다. 상세 로그는 target/T-20/mutation-NN.log에 있다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | nullable conjunction | one_of_requires_exactly_one_matching_branch, invalid_recordings_are_rejected, one_of_requires_exactly_one_matching_branch, invalid_recordings_are_rejected |
| 2 | oneOf count | one_of_requires_exactly_one_matching_branch, one_of_requires_exactly_one_matching_branch |
| 3 | origin comparison | invalid_recordings_are_rejected, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, invalid_recordings_are_rejected, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 4 | method comparison | recordings_replay_over_local_client, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types |
| 5 | metadata conjunction | invalid_recordings_are_rejected, invalid_recordings_are_rejected |
| 6 | schema comparison | recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, invalid_recordings_are_rejected, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, invalid_recordings_are_rejected |
| 7 | envelope direction | recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 8 | depth boundary | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 9 | depth off by one | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 10 | type acceptance | schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 11 | enum acceptance | schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 12 | required presence | schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 13 | allOf ignored | schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected, schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected |
| 14 | properties ignored |  |
| 15 | items ignored | schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected, schema_constraints_and_unsupported_keywords_fail_closed, invalid_recordings_are_rejected |
| 16 | maxLength boundary | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 17 | maxLength comparison | schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client |
| 18 | object type | schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types, schema_constraints_and_unsupported_keywords_fail_closed, recordings_replay_over_local_client, all_recordings_match_current_spec_and_generated_types |
| 19 | array type | schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, schema_constraints_and_unsupported_keywords_fail_closed, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client |
| 20 | string type | schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client |
| 21 | integer type | schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client, schema_constraints_and_unsupported_keywords_fail_closed, one_of_requires_exactly_one_matching_branch, invalid_recordings_are_rejected, all_recordings_match_current_spec_and_generated_types, recordings_replay_over_local_client |
| 22 | number type | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 23 | boolean type | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 24 | null type | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 25 | union intersection | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |
| 26 | extra field accepted | schema_constraints_and_unsupported_keywords_fail_closed, schema_constraints_and_unsupported_keywords_fail_closed |

## 보호 파일 등록

`harness/arch-check/rules.toml`의 broker-toss http-write 허용 파일에 이 작업의 계약 시험 파일과 검증 파일만 추가한다. 녹화 머리 정보의 method 문자열과 잘못된 post 녹화 거부 시험이 범용 HTTP 메서드 표식에 걸리기 때문이다. 제품 코드의 쓰기 전송 허용 범위는 넓히지 않는다. 작업 절의 새 파일 한정 등록 승인에 따른다. 설정·잠금 조각·훅 해시는 공식 생성기로만 재생성한다.

## 검사

- `cargo nextest run --locked -p broker-toss --test contracts`: 5개 통과.
- `cargo clippy --locked -p broker-toss --all-targets -- -D warnings`: 통과.
- `just mutants-diff <최초 기준>`: 최초 시도는 frontendDist 미생성으로 테스트 목록 생성 실패. `just frontend-build` 후 재실행은 위반 없음. 변경된 자동 변이 대상 제품 src가 없어 선택 변이 0개다. 자동 변이 검출 성공 수로 세지 않는다.
- 기존 테스트 변경 및 승인 test-diff 예외: 0건. 기존 impl, 서버 함수, 의존성 변경 없음.

`check-fast`의 전체 승격 첫 실행은 새 validate 함수의 60줄 초과로 clippy에서 실패했다. 길이 검증을 별도 함수로 분리했고 좁은 clippy와 계약5개는 통과했다. 정상 커밋 훅에서 전체를 다시 검사한다.

리뷰 수정: 성공 본문의 구체 생성 타입 재사용을 보강했다. 생성된 환율 타입도 decimal/date-time의 의미는 검사하지 않으며, enum은 unknown을 보존한다. 따라서 원문 enum 대조와 별도의 형식 의미 인계를 유지한다. 쿼리 파라미터 및 봉투 상호 배제의 서술상 계약은 T-05 이후 각 엔드포인트 테스트에 인계한다.
