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
| 통화 불일치 | `cargo nextest run --locked -p domain`, `arbitrary_decimals_preserve_exact_arithmetic_and_currency` | KRW와 USD의 더하기·빼기가 CurrencyMismatch |
| 넘침 | `reviewer_precision_loss_and_overflow_regressions` | MAX·MIN 근처의 정수·소수 합·차를 고정 예제와 독립 256비트 정수 기준으로 대조. 정확한 절댓값이 MAX를 넘으면 Overflow |
| 원금 유지 | `exact_cancellation_and_redundant_scales_remain_valid` | 원화도 소수 8자리 금액을 유지하며 계산 |
| 표시 규칙 | `display_rounds_without_mutating_money_and_direction_uses_original_change` | ±1.235 → ±1.24, −0.004 → 0.00·Down, +0.004 → 0.00·Up, −0.005 → −0.01·Down |
| 표시 경계 | `precision_limits_and_maximum_amount_are_supported` | 0·28자리 허용, 29자리 거부, MAX의 28자리 표시 통과 |
| 라이브러리 표시 경계 | 최초 구현에서 Decimal MAX를 소수 28자리 형식 지정으로 출력 | rust_decimal 내부 CapacityError panic 재현. 기본 문자열 변환 후 0을 붙이는 구현으로 수정. 같은 기대값 유지 |
| 커버리지 | `just coverage-check` | 위반 없음. domain 파일별 100% 정책 유지 |
| 공급망 | `just deny` | advisories ok, bans ok, licenses ok, sources ok |
| 기능 | `cargo tree --locked -p rust_decimal -e features` | 직접 기능 없음. serde·maths·macros·std 비활성 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| Money는 비공개 필드에 공개 전용 타입 Amount와 Currency를 보유하고 checked_add·checked_sub만 제공한다. Currency는 Krw·Usd다 | 통화가 다른 산술은 CurrencyMismatch, 정확한 값이 범위를 넘으면 Overflow, 범위 안의 표현 손실은 PrecisionLoss. 환산은 D-04에 맡긴다 | 통화를 무시한 Decimal 노출 산술, 통화별 제네릭 계층 | architecture 3.4, R-09 |
| Amount는 부호 있는 Decimal newtype이다. 원화도 소수 부분을 보존한다 | 현금 흐름과 손익에는 음수가 필요하며, 통화별 표시 단위가 저장 정밀도를 바꾸면 안 된다 | 원화 생성 때 정수로 반올림, 모든 Money를 음수 금지 | R-11 2장 |
| display_money에서만 명시적 반올림한다. MidpointAwayFromZero, 소수 자릿수 0~28 | ±중간값 처리에 대칭성이 있다. 자릿수는 호출부가 결정한다. 기본 통화별 자릿수·만/억·퍼센트 환산은 이번 범위 밖 | 내부 산술 반올림, Intl 기본 반올림, 통화가 표시 자릿수를 강제 | architecture 3.4·5.3, ADR 0012, [rust_decimal](https://docs.rs/rust_decimal/1.43.0/rust_decimal/) |
| 일반 DisplayMoney에는 방향이 없다. MoneyChange·DisplayMoneyChange만 방향을 가진다. 원래 변화량 부호로 Up·Down을 결정하고 정확한 0만 Flat이다. 표시 숫자만 0이면 부호 없는 0을 쓴다 | 같은 행의 등락액 −3원과 등락률 0.00%도 원래 변화량에서 얻은 Down 하나를 공유한다. 일반 잔고·평가금액에는 상승색을 붙이지 않는다 | 표시 자릿수별로 방향을 따로 계산, 음수 0 | architecture 5.3, DS-05 |
| 표시 결과는 DisplayMoney·DisplayDigits다. Money로 되돌리는 API를 제공하지 않는다 | 반올림 결과를 직접 받는 계산 API는 없다. 다만 as_str → AmountInput → Amount::parse로 우회 재입력하는 길은 구조로 막히지 않는다. 숫자는 ASCII 십진수 문자열이며 UI는 U+2212·쉼표·통화 문구만 형식화한다 | Money를 반올림해서 반환 | architecture 5.3, ADR 0012 |
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
- checked_mul·checked_div·checked_rem은 Some이어도 반올림될 수 있다. saturating 계열도 정확성을 보장하지 않는다. D-02·D-04·D-05·D-06·D-09 등의 후속 작업은 정확 판정 또는 명시적 반올림 정책을 가진 전용 domain API를 만들고 그 파일만 decimal-arithmetic 허용 목록에 올린다. 현재 목록은 비어 있다.
- S-02·S-15 및 S 묶음의 금액 저장 경계에 넘길 요구: 파서는 기존 정규식과 같게 00·01.50·−0을 허용한다. 저장 전 Amount의 십진수 문자열 표현으로 정규화하며, 금액의 대소·동등성을 TEXT 직접 비교로 판단하지 않는다. 후행 0·앞자리 0·음수 0이 값의 동등성을 깨뜨리지 않게 저장 어댑터에서 검증한다.
- Decimal의 유한 96비트·28자리 표현 한계는 그대로다. 무한 정밀도를 제공하지 않는다. 더하기·빼기는 checked 결과가 존재하더라도 정수 계수 대조에서 정확하지 않으면 PrecisionLoss를 반환한다.
- D-02·D-19는 전용 타입을 따로 만들고, D-04는 환율 출처와 시각을 포함한 명시적 환산을 구현한다. D-06의 합계 100% 배분 반올림과 수수료·세금의 절사는 별도 작업이다.
- DS-05는 등락액·등락률에 MoneyChange의 원래 방향 하나를 공유한다. 0으로 표시된 숫자만 음수 부호를 붙이지 않는다. 원래 변화량이 정확히 0일 때만 Flat이다. 퍼센트 표시 함수는 D-02·DS-05에서 이 정책을 따른다.
- 바뀌면 다시 볼 조건: 지원 통화 추가, 표시 중간값 정책 변경, 시간대 DB 필요, Decimal 정밀도 부족 실측, 새 공급망 권고.

## 적대적 리뷰 후 수정

### 정확 산술과 입력 경계

checked_add·checked_sub의 Some도 스케일 축소 반올림일 수 있으므로 그대로 성공으로 쓰지 않는다. 먼저 정확한 크기의 범위를 검사한 뒤 후보는 checked 연산으로 얻고, 정규화한 두 피연산자를 공통 스케일의 i128 정수 계수로 맞춰 정확한 정수 합·차를 계산한다. 후보 결과도 같은 스케일로 옮겨 계수가 같을 때만 성공한다. 정확한 절댓값이 Decimal::MAX를 초과하면 Overflow, 범위 안에서 정수 정렬 또는 후보 대조가 실패하면 PrecisionLoss다.

정규화 후 서로 다른 스케일에서 낮은 스케일의 계수를 확대할 때 i128 범위를 넘으면 높은 스케일의 계수는 96비트 이내라 그 차이를 상쇄할 수 없다. 또한 높은 스케일의 정규화된 계수는 끝이 0이 아니므로 소수 0 제거로 정확히 구제할 수도 없다. 같은 스케일의 두 96비트 계수 합·차는 i128 안에 있다. 따라서 이 정렬 오류는 정확히 표현 가능한 결과를 누락시키지 않는다. 반면 단순 결과 스케일 비교는 큰 수의 .5 + .5가 정확한 정수로 축소되는 경우를 거부하므로 채택하지 않았다. 새 회귀·속성 테스트에 정확한 축소·상쇄도 포함한다.

Amount::parse(&AmountInput)는 ASCII 정규형 `^-?[0-9]+(\.[0-9]+)?$`를 먼저 검사하고 from_str_exact만 호출한다. AmountInput은 경계의 미검증 문자열용 newtype이며, Amount 생성에 성공한 뒤에만 금액으로 다룬다. 지수·밑줄·플러스·공백·범위 및 정밀도 초과를 거부한다. Amount::new(Decimal)은 이미 정확하게 구성한 내부 값용 생성자다.

반올림·절사·스케일 변경과 Decimal 파싱 호출은 xtask source-check의 decimal-method 규칙과 별도 위반 예제로 보호한다. 메서드 이름과 파일 허용 목록은 harness/source-checks/allowlists.toml에 둔다. 반올림은 money_display.rs에서만, 정확 파싱 from_str_exact는 money.rs에서만 허용한다. 손실 파싱 생성자는 그 파일에서도 금지한다. 메서드·파일 원소 하나를 빼는 새 테스트가 각 규칙의 검출력과 허용 범위를 확인한다. Clippy 금지 추가와 expect는 철회했으며 새 검사 끄기는 없다.

검사는 토큰 이름을 보므로 .round 계열은 수신 타입과 무관하게 표시 파일 밖에서 보수적으로 거부한다(ADR 0008의 now 검사와 같은 방식). Decimal·직접 use 별칭·완전한 경로·명시 트레이트 호출·매크로 안 호출과 parse::<Decimal>을 검사한다. TokenStream::from_str와 parse::<u32>는 허용한다.

architecture 5.3은 방향을 코어가 정하며 미정 정책을 D-01에 맡긴다. DS-05는 D-01 ADR을 따르고 0으로 보이는 음수의 부호 처리를 요구한다. UX-1에도 반올림 0을 반드시 Flat으로 만드는 규칙은 없다. 따라서 새 방향 정책과 문서 충돌은 없다.

### 시간 crate 재검토

| 후보 | 직접 확인한 사실 | 판단 |
|---|---|---|
| std::time | 달력 날짜와 고정 오프셋 날짜·시각 타입이 없다 | 단독 사용 불가 |
| time 0.3.55 | `cargo tree --locked -i time`에서 tauri → cookie → time 및 tauri-utils → plist → time 경로 확인. 기존 잠금에 있다. Date·OffsetDateTime은 제공하지만 IANA 지역 시간대 DB는 제공하지 않는다 | 현재 계약은 서버가 거래일과 오프셋을 주므로 선택 유지. [공식 문서](https://docs.rs/time/0.3.55/time/) |
| chrono 0.4.45 | R-09 207행의 typify 조사에서 chrono 의존을 명시한다. 현재 broker-toss-schema는 빈 스켈레톤이라 생성 Rust에서 chrono 사용을 확인한 것은 아니다. 날짜·고정 오프셋은 지원하며 지역 시간대 DB는 별도 chrono-tz가 필요하다 | T-01에서 생성된 chrono 타입은 어댑터 안에서만 취급. domain 타입의 표준을 생성 도구에 맞추지 않는다. [공식 문서](https://docs.rs/chrono/0.4.45/chrono/) |
| jiff 0.2.43 | Timestamp·civil::Date·Zoned·IANA TimeZone과 애매한 지역시각 처리를 제공한다. 잠금 파일에는 있으나(serde_with 선택 의존) 활성 빌드 경로에는 없다 | 향후 임의 시점의 America/New_York 변환이 필요하면 유력하지만 현재 서버 계약에는 추가 DB가 불필요하다. [공식 문서](https://docs.rs/jiff/0.2.43/jiff/) |

확인한 OpenAPI 사본은 docs/toss-openapi-1.2.28.json이다. R-01의 엔드포인트 목록(488~489행)과 대조했다. UsMarketDay.date는 미국 현지 영업일 문자열이며, UsRegularMarketSession.startTime·endTime은 date-time이다. 예제는 2026-03-25T22:30:00+09:00와 다음 날 05:00:00+09:00다. 미국 세션이어도 KST 오프셋을 보낼 수 있으므로 그 시각의 date()를 미국 영업일로 쓰면 안 된다. Candle.timestamp의 1d 설명은 해당 거래일의 현지 자정 고정이다.

- T-01: typify의 chrono 생성물을 실제로 확인하고 경계 변환을 고정한다. 시간은 UTC 초·나노초와 명시 오프셋, 날짜는 연·월·일을 검증해 전용 time 기반 타입으로 변환한다. chrono·serde 타입이 포트·domain으로 새지 않게 한다.
- T-08·D-10: 서버 UsMarketDay.date와 세션 시작·끝을 함께 보존하고 DST 전후(미국 개장 시각이 KST에서 달라지는 녹화 응답)를 검증한다. 개장·휴장일을 자체 시간대 규칙으로 재계산하지 않는다(architecture 3.7, design 4.3).
- D-11·D-27: 일봉 기준일은 응답의 현지 자정 날짜를 원래 오프셋에서 추출해 보존한다. UTC로 바꾼 뒤 date()를 취하지 않는다. 1분봉 구간과 소속 거래일은 서버 캘린더와 대조한다.
- D-31: 미국 일별 스냅샷 키는 UsMarketDay.date이며, KST 자정이나 UTC 자정으로 생성하지 않는다. 캘린더가 없거나 범위를 벗어나 거래일을 확정할 수 없으면 임의 날짜로 귀속하지 않는다. 누락일 재계산 입력과 오류 정책은 D-31에서 고정한다.
- T-08·D-10에서 서버 정보 없이 임의 역사 시점을 지역 날짜에 귀속해야 한다는 요구가 확인되면, 해당 작업의 ADR에서 jiff를 다시 비교한다. 고정 UTC−5·UTC−4 값을 전역 상수로 쓰지 않는다.

D-01에는 시간 타입을 사용하지 않으므로 시간 의존성을 추가하지 않는다. D-09·D-11의 sqrt·powd 등 통계 계산을 구현할 때 R-11에 따라 rust_decimal의 maths 기능을 검토·활성화하고 공급망 검사를 다시 한다. 이번에는 켜지 않는다.

### 추론이 필요한 호출의 한계와 후속 등록

| 구조 검사가 완전히 판정하지 못하는 형태 | 현재 처리·후속 확인 |
|---|---|
| 타입 별칭·복잡한 재수출 별칭 | 직접 use 별칭은 검사한다. 그 밖은 타입 해석이 없어 놓칠 수 있으므로 리뷰에서 확인한다 |
| `let value: Decimal = "1".parse()` 등 추론형 parse | 명시 `parse::<Decimal>`만 토큰으로 검출. 반환 타입 추론이 필요한 호출은 허용하지 않는 코딩 규약과 리뷰로 확인한다 |
| 함수 포인터 변수·일반화한 래퍼 | Decimal::method를 직접 참조하는 경로는 호출 괄호 없이도 검사한다. 변수·제네릭·래퍼를 통한 전달은 타입 해석 없이는 모두 추적하지 못한다 |
| mantissa·scale 등으로 값을 분해하고 재구성 | 금액 계산을 원시 정수로 옮기는 경로를 완전히 검출하지 못한다. domain 전용 API 안의 정밀도 검증 외 재구성은 리뷰에서 확인한다 |
| 표시 문자열을 AmountInput으로 감싸 다시 Amount::parse에 전달 | 타입·구조 검사만으로 막지 못한다. 표시 결과를 저장·계산에 재입력하지 않는 규약이며, 표시 테스트도 이 경로를 사용하지 않는다 |
| 임의 호출의 Decimal 수신 타입 | 산술 검사는 명시 Decimal 타입, 직접 생성자·as_decimal 결과, 지역 바인딩·필드와 그 전달을 추적한다. 임의 외부 함수의 반환·일반화 래퍼는 전면적인 Rust 타입 추론 없이 판정하지 못한다 |

메인이 등록할 것: 이 한계를 just known-gaps 목록에 등록하고 ADR 0021의 요약 개수를 함께 갱신한다. 현재 master의 apps/desktop/tests/known-gaps.test.mjs는 목록 요약이 ADR 0021의 고정 문구와 일치해야 한다고 검사한다. 이번 허용 범위는 기존 ADR 0021 편집을 포함하지 않으므로 자동 수집 제목을 쓰지 않고 이 ADR 본문에만 기록한다. 해당 master 테스트는 변경하지 않는다.

### 마지막 재리뷰의 검증 기준

독립 기준은 테스트 모듈 안의 u64 네 개(256비트)로 원시 96비트 가수를 공통 스케일로 맞춰 더하고 뺀다. 구현의 normalize·i128 정렬·Decimal 산술을 쓰지 않는다. 최대 191비트인 중간 정수는 256비트 안에 들어가며, 정수 10 나눗셈으로만 후행 0을 제거해 96비트·스케일 28의 정확 표현 가능성과 오류 종류를 판정한다. 같은 스케일의 큰 가수, 끝자리 0을 가진 큰 스케일, 상쇄 쌍, MAX 근처를 가중 생성한다. 성공의 정확성과 표현 가능한 값의 성공을 동일 기준으로 검증한다.

D-01의 모든 TestRunner는 Config::with_cases(4096)를 명시한다. 따라서 환경 변수 없이 전체 검사·변형 검사에도 이 횟수가 적용된다. 테스트 헬퍼도 계산 구현에서 독립되어 있으며 M9(같은 스케일이면 대조 생략)와 M3(피연산자 normalize 제거)의 고정 회귀 입력을 포함한다.

범위 분류는 두 피연산자의 절댓값 중 큰 값에서 MAX까지의 여유와 작은 값을 비교한다. 범위 초과 가능성이 있으면 큰 피연산자는 MAX/2 이상이고 Decimal의 96비트 한계상 스케일 0 정수이므로 이 여유의 checked_sub는 정확하다. 큰 값이 소수일 수 있는 작은 크기에서는 두 절댓값의 합이 MAX에 닿지 않아 오분류하지 않는다. MAX+0.4와 MAX+0.5 모두 Overflow이며 MAX−0.4 같은 범위 안의 비표현 값은 PrecisionLoss다.

rust_decimal 1.43.0의 로컬 arithmetic_impls.rs와 Decimal 정의에서 확인한 목록은 checked_mul·checked_div·checked_rem, saturating_mul·saturating_add·saturating_sub 및 Mul·Div·Rem·대입 트레이트 메서드다. wrapping_*·overflowing_* Decimal 메서드는 이 버전에 없다. 구조 검사는 산술 목록을 별도의 빈 decimal-arithmetic 파일 허용 목록으로 관리하고 to_f32·to_f64는 허용 파일 없이 금지한다. 정수 계수·테스트 큰 정수의 checked 연산은 Decimal 산술과 구분한다.

jiff 상태는 Cargo.lock에 이름이 있는 것과 활성 빌드 경로를 분리해 확인했다. cargo tree --locked -i jiff는 nothing to print를 출력한다. 현재 활성 기능 선택에서 사용하지 않는 serde_with의 선택 의존이 잠금에 남아 있는 상태다.
