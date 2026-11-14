# ADR 0053: T-01 토스 스키마 생성

- 상태: 확정 (2026-11-14)
- 기준: - 상태: 적대적 리뷰 수정 초안
- 최초 기준: ac1232699397ba824a201a9994dec7f44f3f0230
- 리뷰 대상: d0e05a01c72405012e4d2eb172df2e1ce4d6e2b3

## 생성과 파일 경계

로컬 OpenAPI 1.2.28의 components.schemas 109개만 typify 0.8.0에 전달한다. 참조 경로만 JSON Schema definitions로 옮긴다. xtask gen-schema가 Cargo.toml, src/lib.rs, src/types.rs를 생성한다. --check는 세 파일의 바이트를 모두 비교하고, crate 전체의 실제 파일·폴더·링크와 git index를 확인한다. 이 세 파일과 src 폴더 외 항목은 무시 파일이나 작업 사본에서 삭제된 추적 파일도 거부한다. 생성기는 외부 파일을 삭제해 맞추지 않는다. 입력 원문 SHA-256으로 공백 한 글자의 변화도 드러낸다. 생성기는 실행 시 의존성이 아니다.

스키마 설명과 모든 doc 속성을 제거한 뒤 Rust AST를 정리한다. 패턴 검증의 LazyLock과 unwrap은 정규식 생성 실패를 Result로 반환하는 코드로 바꾼다. 패턴 제약을 삭제하지 않는다. 지역 식별자, 숫자 접미사, 문자열 소유 복사와 불필요한 self::error 경로를 정리한다. 열거형 기본값은 생성된 Default 구현에서 읽어 같은 변형의 derive로 옮긴다. 11개 구조체의 Rust 필드명에서 반복 접두사·접미사를 제거하되 serde의 JSON 이름은 보존한다. 새 검사 끄기는 없다.

## 원시 값과 후속 변환

format=decimal은 JSON 문자열 형식 표식이다. 120곳(문자열 91곳, nullable 문자열 29곳)을 원문 문자열로 보존하며, 이 중 111곳은 숫자 패턴이 없어 길이만 검사한다. 따라서 abc 같은 값도 생성 타입에서 받아들일 수 있다. f64 생성 필드는 없다. 스키마의 패턴 제약 12곳은 그대로 검사한다.

T-04 이후의 어댑터는 생성 문자열을 받은 즉시 Money::parse 등 도메인 생성자로 변환해야 한다. 변환 전 문자열을 금액이나 수량으로 사용하는 경로를 만들지 않는다. 원시 i64는 범위만 표현하며 음수 accountSeq 등의 의미 유효성을 증명하지 않는다. 이 값도 도메인 생성자와 확정한 식별자 정책을 거친다. 문서 근거 없이 새 양수 정책을 만들지 않는다.

## 미지 열거값

문자열 열거형은 알려진 단위 변형과 Unknown(String)을 갖는다. From<String>과 FromStr, serde 역직렬화는 알려진 값과 미지 값을 구분하며, 직렬화는 미지 원문을 그대로 돌려준다. Vec<Currency>의 KRW/EUR/USD와 Transaction 정상 두 행·미지 통화 및 거래 종류 한 행을 함께 왕복하는 실제 타입 테스트로 목록 전체가 실패하지 않음을 검증한다.

이것이 domain의 Currency를 확장하는 것은 아니다. ADR 0034에 따라 T-06은 Unknown 통화 행을 원문과 함께 격리·보존하고 알린다. 미지 통화를 KRW/USD로 치환하거나 그 행만 빼고 완전 응답으로 만들면 안 된다. 격리 행이 있으면 CompleteTransactionResponse를 만들지 않고 사라짐 대조도 호출하지 않는다. 미지 거래 종류·상품은 도메인의 Unknown 정책으로 변환한다. T-04·T-06·T-16은 생성 타입 디코드 성공과 도메인 검증 성공을 구분해야 한다.

## 날짜와 주문 요청

날짜 25곳과 시각 40곳은 문자열 및 nullable을 보존한다. broker-toss는 ADR 0028과 같은 time 0.3.55를 std/parsing 기능으로 사용한다. 생성 타입에 chrono를 사용하지 않으며 어댑터의 chrono 직접 의존과 bans 허용을 제거한다. time의 파싱 전에 YYYY-MM-DD, 대문자 T/Z, 고정 자릿수 시·분·초와 오프셋을 검사한다. 윤초·-00:00·9자리 초과 소수 초를 오류로 거부하며, 검증한 구성 요소만 domain에 넘긴다. T-08의 세 행 캘린더 조립은 별도다.

untagged OrderCreateRequest에는 deny_unknown_fields를 적용한다. quantity와 orderAmount가 모두 있는 입력은 어느 변형으로도 받아들이지 않는다. 수량형·금액형의 단독 입력과 양쪽 동시 입력을 회귀로 고정한다. O 절과 T-16은 이 생성 타입을 도메인 주문 검증이나 사람 승인 대신 사용하지 않는다.

## 공개 표면과 검사

broker-toss·broker-toss-trade의 공개 재수출, 함수·메서드 시그니처, 구조체의 공개 필드, 열거형 변형, 타입 별칭, trait 연관 타입·시그니처와 제네릭 내부에 생성 타입이 있으면 arch-check가 실패한다. Cargo 메타데이터의 의존성 이름 변경을 시작 이름으로 받고, 파일 간 use 별칭·타입 별칭을 고정점으로 추적한다. 비공개와 pub(crate) 사용은 허용한다. 이는 crate 단위 보수적 정적 검사이며 Rust 컴파일러 전체 이름 해석이나 외부 절차 매크로 확장기를 구현한 것은 아니다.

재생성 검사는 just check의 Rust 생성물 검사 위치에서 수행한다. schema에는 형식·Clippy가 적용되고, 커버리지와 cargo-mutants는 기존 정책 대상 밖이다. 생성기 테스트는 xtask, 생성 타입 테스트는 두 허용 어댑터 안에 둔다.

## 메인 인계와 미해결

- T-04 이후: 길이만 검사된 십진 문자열과 원시 i64의 도메인 변환을 강제한다.
- T-06: 미지 통화의 격리·보존·알림과 완전 응답 금지 계약을 적용한다.
- O 절·T-16: 요청의 상호 배타와 도메인 검증·사람 승인을 유지한다.
- T-20: 객체 example이 없는 스키마의 완전 응답과 녹화 계약을 검증한다.
- generated-attribute allow 제거는 master의 generated_code_is_exempt_only_from_what_it_emits 테스트가 allow/doc 목록을 고정하는 것과 충돌한다. 기존 테스트와 test-diff를 변경하지 않았다. 메인이 이 테스트 기대값을 먼저 갱신해야 예외 항목을 제거할 수 있다. 실제 생성물의 allow와 doc는 모두 0개다.
- 메인이 harness.md 3.5에 반영할 문장: xtask의 외부 의존은 cargo_metadata, syn(full/visit/visit-mut), proc-macro2, ra-ap-rustc_lexer, toml, typify(기본 macro 기능 끔), schemars, serde_json, quote, sha2다. 작업 공간 crate에는 의존하지 않는다.
