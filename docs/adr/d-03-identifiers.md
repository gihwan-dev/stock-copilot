# D-03 식별자 형식과 계좌 표시 경계

- 상태: 초안, 메인 검토 대기
- 관련 작업: D-03
- 기준 커밋: 7681ec8a985cc8aa882a9302dde035ef1cfcc7a6

## 근거와 결정

| 타입 | 근거 | 구현 |
|---|---|---|
| Symbol | R-01 출처 S5의 저장소 사본 `docs/toss-openapi-1.2.28.json`, `components.parameters.Symbol`, `SymbolQuery`, `KrSymbol` | 비어 있지 않은 ASCII 영숫자·점·하이픈. 명시 패턴 `^[A-Za-z0-9.\-]+$`을 따른다. 대소문자와 길이를 보존한다. 국내 코드 6자리 설명을 모든 종목에 강제하지 않는다. R-07 종목 마스터의 영숫자 코드 실측과도 일치한다 |
| Market | 같은 S5 사본 `components.schemas.MarketCountry` | KR·US만 생성한다. 알 수 없는 외부 enum은 어댑터가 별도 Unknown으로 받고 지원 시장으로 변환하지 않는다. 원시 문자열 파싱을 생성자 한곳에 제한하기 위해 비공개 문자열 newtype을 쓴다 |
| ClientOrderId | R-01 Q7의 중복 방지 키 절 | 1~36자, ASCII 영숫자·하이픈·밑줄. UUID 형식을 강제하지 않는다. 생성·유일성·10분 재시도 정책은 이 값 타입의 범위 밖이다 |
| InstrumentId | architecture 3.4, R-11 기업행위 표의 종목 코드·이름 변경 행 | Symbol과 다른 불투명 내부 식별자. 생성자는 비어 있지 않은 ASCII 표시 문자만 받는다. Symbol·ISIN에서 파생하거나 코드 변경에 따라 재발급하지 않는다 |
| AccountId | SEC-7, R-01 6장 계좌 엔드포인트와 S5 사본 `components.schemas.Account` | accountNo를 보관하는 비공개 newtype. 임시로 ASCII 숫자 5자리 이상만 받는다. 뒤 네 자리보다 긴 값만 허용해 전체 번호가 표시되는 경우를 막는다. 오류에는 입력값을 넣지 않는다. 표시 접근자·Display·Debug 모두 뒤 네 자리만 반환한다. 전체 값 접근자·serde 의존·직렬화 구현은 두지 않는다 |

## 미해결

- Account.accountNo에는 문자열 타입과 예시만 있고 길이·패턴 규칙이 없다. 숫자 5자리 이상은 공식 API 규칙이 아니라 노출 방지를 우선한 임시 제한이다. 11자리 고정, 하이픈 제거, 공백 제거, 체크섬은 추측하지 않는다. 실제 번호를 사용하지 않은 어댑터 계약 확인 뒤 재검토한다.
- 지어낸 하이픈 표기 `123-45-678901`은 `InvalidFormat`으로 거부한다. 하이픈을 제거해 수용하지 않는다. `accounts_reject_hyphenated_notation_without_normalizing_it`가 이 동작을 고정한다. 토스 응답에 하이픈 표기가 가능한지는 미확정이며, 정규화 정책은 어댑터 계약 확인 뒤 정한다.
- API 헤더·웹소켓은 accountNo가 아니라 accountSeq를 쓴다. AccountId를 accountSeq로 대신 보내면 안 된다. accountSeq 타입과 번호 사이 매핑은 계좌 어댑터 작업에서 정한다.
- InstrumentId의 발급 형식·길이·유일성 보장은 문서에 없다. UUID나 데이터베이스 키라고 가정하지 않는다. ASCII 표시 문자 제한은 공백·제어 문자·숨은 문자 혼입을 거부하는 임시 경계다. 영구 저장 정책이 정해질 때 재검토한다.
- 계좌 전체 번호를 DB로 복원하는 경계는 아직 없다. 일반 직렬화를 허용하지 않고 추후 경계 어댑터에서 별도 설계한다. domain에 serde를 넣지 않는 architecture 3.2 규칙을 유지한다.

## 대안

- 계좌에 Debug·Serialize를 자동 파생하면 전체 값이 노출되므로 채택하지 않는다.
- AccountId를 뒤 네 자리만 저장하면 서로 다른 계좌가 같은 ID가 되므로 채택하지 않는다.
- Symbol을 숫자 또는 6자리로 제한하거나 InstrumentId와 합치면 외국 티커·영숫자 코드·코드 변경을 처리하지 못하므로 채택하지 않는다.

## 검증

네 모듈의 단위 테스트는 형식 실패·경계 길이·종목 코드 보존·계좌 표시·동일 접미사의 별도 식별성을 확인한다. 외부 소비자 컴파일 실패 검사는 비공개 필드 생성과 serde 직렬화 불가를 확인한다. 컴파일 실패 검사의 임시 파일은 작업 사본 target 아래에만 두고 제품 의존성에 serde를 추가하지 않는다.

컴파일 실패 회귀 테스트 방식도 조사했으나 현재 허용 범위에서는 추가하지 않았다. `crates/xtask/src/rust_lints_tests.rs`는 자식 cargo 실행으로 컴파일 진단을 검증하지만 보호 파일이며, domain은 IO와 serde 의존을 허용하지 않는다. 문서가 권하는 trybuild는 현재 Cargo.lock과 deny.toml 허용 목록에 없다. doctest의 compile_fail은 문서 주석 또는 doc 속성이 필요하여 harness 2장·3.2의 금지와 충돌하고, just check의 nextest 실행에도 포함되지 않는다. 보호 파일·의존성 정책을 바꾸지 않는 범위에서는 임시 컴파일 실패 증거만 유지한다. 영속 회귀 테스트를 위한 하네스 지원은 메인 결정이 필요하다.
