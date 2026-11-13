# D-03 식별자 형식과 계좌 표시 경계

- 상태: 초안, 메인 검토 대기
- 관련 작업: D-03, 적대적 리뷰 수정
- 최초 기준: 7681ec8a985cc8aa882a9302dde035ef1cfcc7a6
- 수정 기준: 62fc1770a3a11a49f08edd9af5e33a1da7f1162b

## 근거와 결정

| 타입 | 근거 | 구현 |
|---|---|---|
| Symbol | R-01 출처 S5의 사본 `docs/toss-openapi-1.2.28.json`의 `components.parameters.Symbol`, `SymbolQuery`, `KrSymbol`과 메인의 리뷰 판정 | 첫 글자는 ASCII 영숫자, 전체는 ASCII 영숫자·점·하이픈, 1~32자. 스펙의 `^[A-Za-z0-9.\-]+$`보다 좁힌다. 점 경로 `..`가 URL 정규화로 조회 경로를 바꾼 재현에 따라 점·하이픈으로 시작하는 코드를 거부하고 입력 크기를 제한한다. 6자리 숫자로 제한하지 않는다 |
| Market | S5의 `components.schemas.MarketCountry`, architecture 3.4의 상태와 외부 열거값 규칙, design API-7 | `Kr`, `Us`, `Unknown(MarketCode)` 열거형. architecture 3.4는 외부 열거값에 Unknown을 명시적으로 요구한다. 모르는 외부 코드를 버리거나 알려진 시장으로 바꾸지 않는다 |
| MarketCode | harness 3.4의 newtype 생성자·접근자 예외 | 비어 있지 않은 ASCII 표시 문자열의 경계 타입. `MarketCode::try_new`에서 형식을 검증하고 `Market::new`에서 열거형으로 해석한다. `Market::as_code`는 MarketCode를 반환하며 그 `as_str`에서 문자열을 얻는다. 열거형 자체에 원시 타입 시그니처를 두지 않는다 |
| ClientOrderId | R-01 Q7의 중복 방지 키 절 | 1~36자, ASCII 영숫자·하이픈·밑줄. UUID 형식을 강제하지 않는다. 발급·유일성·10분 재시도 정책은 이 값 타입의 범위 밖이다 |
| InstrumentId | architecture 3.4, R-11 기업행위 표의 코드·이름 변경 행, 메인의 리뷰 판정 | Symbol과 독립된 불투명 내부 ID. 비어 있지 않은 ASCII 표시 문자 1~64자. 코드·ISIN에서 파생하거나 코드 변경에 따라 재발급하지 않는다 |
| AccountId | R-01 6장의 계좌 엔드포인트, S5의 `Account.properties.accountSeq`와 `parameters.AccountSeq` | accountSeq를 감싼 i64 newtype. 스펙은 integer/int64만 정하고 minimum·maximum을 추가하지 않았다. signed int64 전체 범위를 타입으로 보장하며 양수 제약을 추측하지 않는다. Eq·Hash·Ord를 제공하는 domain 계좌 키다 |
| MaskedAccountNumber | design SEC-7, 메인의 리뷰 판정 | 전체 accountNo를 받아 검증한 뒤 뒤 네 자리만 보관한다. 앞자리는 복제하거나 저장하지 않는다. 입력은 ASCII 숫자 8~20자리이며 하이픈은 거부한다. 표시 형식은 Display의 네 자리 숫자 하나이고 별도 가림 기호를 붙이지 않는다. 원시 문자열 접근자는 없다. Debug·비교도 이미 잘린 값만 다룬다 |

## 계좌 키와 개인정보

SEC-7은 화면·AI 프롬프트의 계좌번호를 뒤 네 자리로 제한한다. S5는 accountNo를 계좌번호로, accountSeq를 API 요청에 쓰는 별도의 계좌 식별 키로 정의한다. 따라서 이 가림 요구는 MaskedAccountNumber에 적용하고 AccountId의 숫자 키를 네 자리로 자르지 않는다. 숫자 키를 화면이나 AI에 보내야 한다는 뜻은 아니며 현재 타입에는 사용자 표시용 Display를 제공하지 않는다. 키를 비교·정렬·해시해도 전체 계좌번호를 추측할 수 있는 번호 데이터가 없다. 표시 타입은 같은 접미사와 다른 접두사의 입력을 구분하지 않는다.

전체 accountNo를 저장해야 하는 요구가 있는지 design·architecture와 tasks를 검색했다. design 2.5의 계좌 선택과 시스템 설정, tasks U-22의 첫 종합매매 계좌 자동 선택·설정 변경은 accountSeq 키와 마스킹된 번호로 구현할 수 있다. architecture 5장의 저장소 요구에서도 전체 계좌번호 보관을 요구하는 항목은 찾지 못했다. 원본 보존 대상인 거래내역과 별도로 정의된 계좌 키·표시 번호를 혼동하지 않는다. 문서 충돌은 발견하지 못했으며 후속 저장소 스키마가 전체 번호를 전제하면 이 결정과 대조해야 한다.

## 미해결과 후속 범위

- Account.accountNo에는 문자열 타입과 예시만 있고 길이·패턴 규칙이 없다. ASCII 숫자 8~20자리는 메인이 지정한 임시 정책이며 공식 API 제약이 아니다. 지어낸 `123-45-678901`은 InvalidFormat으로 거부하고 하이픈을 제거하지 않는다. 토스의 하이픈 응답 여부와 정규화 정책은 어댑터 계약 확인 뒤 정한다.
- domain은 Symbol·MarketCode의 대소문자를 정규화하지 않는다. 토스의 소문자 처리 동작은 미실측이며 어댑터 경계에서 정한다. 소문자 시장 코드도 원문을 보존한 Unknown이다.
- Symbol과 Market의 조합 검증, ASCII 영숫자인 OCC 옵션 심볼 `AAPL261218C00150000`의 상품 구분은 D-08·어댑터 작업으로 넘긴다. 현재 Symbol은 문법만 검증하며 주식 여부·실제 상장 여부를 보장하지 않는다.
- InstrumentId의 발급 형식과 유일성 보장은 미정이다. UUID나 데이터베이스 키라고 가정하지 않는다.
- 컴파일 실패 회귀 테스트는 메인이 별도 하네스 작업으로 넘겼다. 기존 xtask 방식은 보호 파일이고 trybuild는 현재 허용 목록에 없다. doctest compile_fail은 문서 주석·doc 속성 금지와 충돌하며 just check의 nextest에도 포함되지 않는다. 이번 수정의 영속 회귀 테스트에는 포함하지 않는다.
- proptest 속성 테스트는 D-01 브랜치의 의존성 허용 목록 반영 뒤 후속으로 진행한다. 이번에는 구체적인 경계값·리뷰 반례와 변형 검사로 확인한다.

## 검증

Symbol은 점 경로·점과 하이픈 접두사, 32/33자리 경계와 BRK.B·BRK-B·0101N0을 검증한다. InstrumentId는 64/65자리와 10MB 입력을 검증한다. AccountId는 i64 최솟값·음수·0·양수·최댓값을 보존하고 정렬·해시 키로 쓴다. MaskedAccountNumber는 8/20자리 통과와 7/21자리 실패, 숫자 이외 문자·하이픈 거부, 내부 저장값 자체가 네 자리임을 검증한다. 표시·Debug와 동일 접미사의 비교 결과도 테스트한다. Market은 알려진 두 변형과 모르는 외부 코드의 왕복을 검증한다.

수정 구현의 단위 테스트 16개, clippy, raw-types, coverage-check가 통과했다. `just mutants-diff 62fc1770a3a11a49f08edd9af5e33a1da7f1162b`는 domain 전체 변형 56개를 검사해 50개 검출, 6개 컴파일 불가, 생존 0개로 끝났다. Symbol·InstrumentId·표시 번호의 길이 경계 비교와 문자 검사 조건의 변형도 검출했다. 이 검사는 생성한 변형의 검출 결과이며 모든 가능한 결함의 부재를 증명하지 않는다.
