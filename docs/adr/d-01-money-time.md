# ADR 초안 D-01: 금액, 표시 반올림과 시간 타입

- 상태: 제안
- 날짜: 2026-11-13
- 결정한 사람: 구현 담당(메인 평가 대기)
- 관련 작업: D-01, 후속 D-02·D-04·D-06·D-10·DS-05
- 반영한 문서: 없음. 병합 때 메인이 번호와 참조를 정한다.

## 맥락

architecture 3.4는 통화가 다른 금액 합산을 거부하고, 십진수 산술은 checked 계열만 쓰며, 저장·중간 계산에는 표시 반올림을 적용하지 않도록 정한다. 5.3과 8장은 표시 후 0이 된 등락의 방향과 시간 crate 선택을 D-01에 맡긴다. R-09는 rust_decimal 1.43.0을, R-11은 표시 시점의 반올림을 지정한다. ADR 0012는 TypeScript의 암묵적 반올림을 제거했다.

## 실측

환경: 저장소 고정 Rust 1.99.1, macOS, cargo-nextest 0.9.151.

| 항목 | 명령·테스트 | 결과 |
|---|---|---|
| 통화 불일치 | `cargo nextest run --locked -p domain`, `different_currencies_cannot_be_added_or_subtracted` | KRW와 USD의 더하기·빼기가 CurrencyMismatch |
| 넘침 | `checked_arithmetic_reports_overflow_for_every_positive_increment` | proptest가 만든 양의 정수 증가량에 대해 MAX 더하기·MIN 빼기가 Overflow |
| 원금 유지 | `same_currency_arithmetic_preserves_fractional_amounts` | 원화도 소수 8자리 금액을 유지하며 계산 |
| 표시 규칙 | `display_rounds_midpoints_away_from_zero_without_changing_money` | ±1.235 → ±1.24, ±0.004 → 0.00·Flat, −0.005 → −0.01·Down |
| 표시 경계 | `precision_is_bounded_and_currency_does_not_round_storage` | 0·28자리 허용, 29자리 거부, MAX의 28자리 표시 통과 |
| 라이브러리 표시 경계 | 최초 구현에서 Decimal MAX를 소수 28자리 형식 지정으로 출력 | rust_decimal 내부 CapacityError panic 재현. 기본 문자열 변환 후 0을 붙이는 구현으로 수정. 같은 기대값 유지 |
| 커버리지 | `just coverage-check` | 위반 없음. domain 파일별 100% 정책 유지 |
| 공급망 | `just deny` | advisories ok, bans ok, licenses ok, sources ok |
| 기능 | `cargo tree --locked -p rust_decimal -e features` | 직접 기능 없음. serde·maths·macros·std 비활성 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| Money는 비공개 Amount와 Currency를 보유하고 checked_add·checked_sub만 제공한다. Currency는 Krw·Usd다 | 통화가 다른 산술은 CurrencyMismatch, Decimal checked 결과 None은 Overflow. 환산은 D-04에 맡긴다 | 통화를 무시한 Decimal 노출 산술, 통화별 제네릭 계층 | architecture 3.4, R-09 |
| Amount는 부호 있는 Decimal newtype이다. 원화도 소수 부분을 보존한다 | 현금 흐름과 손익에는 음수가 필요하며, 통화별 표시 단위가 저장 정밀도를 바꾸면 안 된다 | 원화 생성 때 정수로 반올림, 모든 Money를 음수 금지 | R-11 2장 |
| display_money에서만 명시적 반올림한다. MidpointAwayFromZero, 소수 자릿수 0~28 | ±중간값 처리에 대칭성이 있다. 자릿수는 호출부가 결정한다. 기본 통화별 자릿수·만/억·퍼센트 환산은 이번 범위 밖 | 내부 산술 반올림, Intl 기본 반올림, 통화가 표시 자릿수를 강제 | architecture 3.4·5.3, ADR 0012, [rust_decimal](https://docs.rs/rust_decimal/1.43.0/rust_decimal/) |
| 표시 금액이 0이면 방향 Flat, 숫자 부호도 양의 0이다. 나머지는 표시 값 부호로 Up·Down을 결정한다 | 0.00과 하락색·음수 부호가 충돌하지 않는다. 원본 금액은 그대로 유지하므로 계산·신호 판정에는 영향이 없다 | 표시 0에도 원본의 Up·Down을 보존, 음수 0 | architecture 5.3, DS-05 |
| 표시 결과는 DisplayMoney·DisplayDigits다. Money로 되돌리는 API를 제공하지 않는다 | 반올림 결과가 계산 API에 바로 들어가지 않는다. 숫자는 ASCII 십진수 문자열이며 UI는 U+2212·쉼표·통화 문구만 형식화한다 | Money를 반올림해서 반환 | architecture 5.3, ADR 0012 |
| 시간 crate는 time을 선택한다. D-01에는 의존성을 추가하지 않는다 | UTC 시점·날짜·고정 오프셋·기간을 다룰 수 있고 기능을 분리해 켤 수 있다. D-01 금액 계산은 시간 타입을 사용하지 않는다 | std만 사용, chrono 선택 | architecture 8장, [time 0.3.55](https://docs.rs/time/0.3.55/time/) |

### 시간 타입 후보 비교

| 후보 | 장점 | 제한·판단 |
|---|---|---|
| std::time | 추가 의존성 없음 | 달력 날짜·고정 오프셋 표현이 없어 도메인 날짜 처리에 부족 |
| chrono | 날짜·시점·시간대 유형과 검증된 생태계. R-09의 스키마 도구 의존성 설명에도 등장 | 기본 clock·now 기능을 끄고 선택할 수 있으나 이번 요구에 IANA 시간대 체계는 필요하지 않음. 선택하지 않음. [공식 문서](https://docs.rs/chrono/0.4.45/chrono/) |
| time | Date·OffsetDateTime·Duration, 파싱·형식·로컬 오프셋 기능 분리 | IANA 시간대 DB는 제공하지 않는다. 장 운영 시각은 토스 정보와 Clock 포트로 받는 현재 구조에 맞아 선택 |

후속 시간 타입 도입 때 `default-features = false`로 시작하고 사용 기능과 정확한 버전을 고정한다. 이번 조사 기준 문서 버전은 time 0.3.55이며, 실제 도입 시 호환성과 공급망을 재검사한다. 시각은 전용 타입으로 감싸고 현재 시각은 Clock에서 받는다. 시간 crate 선택이 SystemTime·Instant·now 직접 호출 허용을 뜻하지 않는다. D-10에서 거래소 현지 달력 계산이 필요해지면 서머타임 요구를 다시 검토한다.

### 의존성 변경

| 직접 crate | 버전 | 직접 켠 기능 | 전이 패키지 수 | 라이선스 |
|---|---|---|---|---|
| rust_decimal | =1.43.0 | 없음, 기본 기능 끔 | 3 | MIT |
| proptest (개발 전용) | =1.12.0 | std만, 기본 기능 끔. fork·timeout 비활성 | 21 | MIT OR Apache-2.0 |

전이 수는 `cargo metadata --locked`에서 각 crate의 일반·빌드 의존 경로를 따라 세었다. 전체 대상과 작업 공간 기능 통합을 포함하므로 macOS 단독 빌드보다 넓다. 두 집합을 합친 전이는 22개이고, 직접 2개를 합치면 총 24개다. 기존 잠금 대비 신규 패키지는 직접 2개와 전이 8개다.

새 잠금 패키지: arrayvec 0.7.8, chacha20 0.10.2, core_detect 1.0.0, cpufeatures 0.3.1, proptest 1.12.0, rand 0.10.3, rand_core 0.10.1, rand_xorshift 0.5.0, rust_decimal 1.43.0, unarray 0.1.4. rust_decimal은 MIT이고 나머지는 MIT/Apache-2.0 선택 라이선스다. num-traits 0.2.19는 기존 잠금에 있었으나 이번에 실제 경로가 추가되어 허용 목록에 추가한다. num-traits와 rust_decimal의 build.rs만 추가로 허용한다.

구조 검사의 domain 외부 일반 의존 허용 목록은 그대로 rust_decimal 하나다. 개발 의존 proptest는 기존 검사에서 제외되므로 목록을 넓히지 않는다. 작업 공간 기능 통합의 `proptest → bitflags → serde_core → serde_derive → syn·proc-macro2` 경로에만 위치 예외를 추가한다. domain은 serde 기능을 켜지 않는다.

## 결과

- Money에 IO, 시계, 직렬화, 주문 전송 경로를 추가하지 않는다.
- Decimal의 유한 96비트·28자리 표현 한계는 그대로다. 무한 정밀도나 모든 소수 연산의 수학적 정확성을 보장하지 않는다. 이번 API는 더하기·빼기만 제공하며 Decimal의 checked 결과를 전달한다.
- D-02·D-19는 전용 타입을 따로 만들고, D-04는 환율 출처와 시각을 포함한 명시적 환산을 구현한다. D-06의 합계 100% 배분 반올림과 수수료·세금의 절사는 별도 작업이다.
- DS-05는 표시 0을 Flat으로 처리하고 음수 부호를 붙이지 않는다. 퍼센트 등 다른 단위의 값은 해당 도메인 표시 함수가 같은 정책을 적용해야 한다.
- 바뀌면 다시 볼 조건: 지원 통화 추가, 표시 중간값 정책 변경, 시간대 DB 필요, Decimal 정밀도 부족 실측, 새 공급망 권고.
