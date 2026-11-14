# T-01: 토스 스키마 생성

- 상태: 초안
- 기준: ac1232699397ba824a201a9994dec7f44f3f0230

## 결정

로컬 OpenAPI 1.2.28의 components.schemas 109개만 typify 0.8.0에 전달한다. 참조 경로만 JSON Schema definitions로 옮긴다. xtask gen-schema가 lib.rs와 types.rs를 만들고 커밋한다. --check는 임시 재생성 결과와 파일 바이트를 비교한다. 입력 원문 SHA-256을 lib.rs에 기록해 공백 한 글자의 변화도 드러낸다. 실행 시 typify를 사용하지 않는다.

스키마 설명을 제거한 뒤 Rust AST를 정리한다. typify가 만드는 패턴 검증의 LazyLock과 unwrap을 매번 생성하는 정규식과 Result 오류로 바꾼다. 검증을 삭제하지 않는다. 지역 식별자, 숫자 접미사와 문자열 소유 복사 표기를 바꾸고 불필요한 self::error 경로를 없앤다. DAY 기본값은 동일한 Default derive로 옮긴다. 11개 구조체의 Rust 필드명에서 반복되는 접두사·접미사만 제거하고 serde의 JSON 필드명은 보존한다. 기존 검사 끄기 보고 테스트를 유지하며 새 expect는 추가하지 않는다. 원시 생성물을 손으로 편집하지 않는다.

decimal 형식 문자열 120곳(91개 필수형 문자열, 29개 nullable 문자열)은 십진 문자열로 보존한다. number 스키마 및 f64 생성 필드는 0개다. 패턴 제약 12곳도 보존한다. Decimal 변환과 금액 계산은 후속 어댑터와 domain의 책임이다.

날짜 25곳과 시각 40곳은 문자열로 생성한다. chrono의 기본 DateTime<Utc> 생성은 원래 오프셋을 잃으므로 사용하지 않는다. broker-toss의 TossDate와 TossTimestamp가 검증한 구성 요소만 domain 시간 타입에 전달한다. 윤초, -00:00, 9자리를 넘는 소수 초는 오류다. 시각을 읽거나 시간대를 추정하지 않는다. T-08의 세 행 캘린더 조립은 이 작업에 포함하지 않는다.

스키마 열거형 37곳은 typify의 닫힌 열거형을 유지한다. 모르는 값은 역직렬화 오류다. 후속 broker-toss와 broker-toss-trade의 응답 매핑은 먼저 원시 serde_json::Value를 보유하고, 원시 문자열을 읽어 domain의 Unknown으로 옮겨야 한다. typed decode 실패로 원시 응답을 버리거나 기존 변형에 끼워 맞추지 않는다. 원문을 유지하는 방법을 계약 테스트로 고정한다. 현재 HTTP 응답 매핑은 T-04 이후의 범위다.

생성 경로, raw-types 경계, 의존 간선과 부정 사례는 기존 정확 경로 정책을 사용한다. schema는 Clippy와 형식 검사 대상이다. 커버리지 정책과 변이 대상은 domain/order-execution/application이며 schema는 대상이 아니다. 생성기 테스트는 xtask, 생성 타입 왕복 테스트는 broker-toss에 둔다.

## 미해결

- 필드 수준 example만 있고 객체 example이 없는 스키마의 완전 응답 조립 및 녹화 응답 검증은 T-20에서 수행한다. 스키마 자체의 example 세 개는 모두 왕복한다.
- 실제 HTTP 응답의 Unknown 분류와 저장 정책은 T-04·T-05·T-16에서 위 원시 보존 계약을 적용해야 한다.
- 프로젝트·훅의 T3 실행 신뢰 승인 상태는 이 실행에서 독립적으로 증명할 수 없다. 저장소 설정과 설치된 git 훅 및 명시적 check 결과를 확인한다.
