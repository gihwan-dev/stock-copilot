# D-20 초안: 투자 성향의 선택지와 기본값

- 상태: 초안, 종목 유형 목록과 보수적 경계는 사용자 확인 대상
- 기준: d4ab5a2287b04757ca63066afa8e57c8bd232622
- 근거: design 6.2·8장·SEC-12, open-issues 1.2·1.3·P-4, tasks D-20, ADR 0022·0025·0028·0029, 이번 메인 결정

## 값 집합과 기본값

| 구성 요소 | 닫힌 값 집합 또는 범위 | 기본값과 근거 |
|---|---|---|
| HorizonSelection | FewDays(며칠), WeeksToMonths(몇 주~몇 달), HalfYearOrMore(반년 이상), Hours(몇 시간 단위) | WeeksToMonths. design 6.2의 기간 기본값 “몇 주~몇 달” |
| RiskAppetite | Conservative(보수), Neutral(중립), Aggressive(공격) | Neutral. design 6.2 “보수 / 중립 / 공격”, 기본값 “중립” |
| Exclusions | SectorId 목록과 ExcludedSecurityType 목록. 중복을 거부하고 입력 순서를 보존 | 빈 두 목록. design 6.2 “종목 유형, 섹터”, 기본값 “없음”. 개별 종목·시장 제외를 추가하지 않음 |
| ProfileWeight | Ratio를 감싼 전용 타입. 0 초과 1 이하 | 종목 상한 0.2, 주의 기준 0.1, 섹터 상한 0.4. design 6.2 “종목 20%(10%부터 주의), 섹터 40%” |
| WeightCaps | 주의 기준 ≤ 종목 상한 ≤ 섹터 상한 ≤ 100% | 위 세 기본값. 서로 같은 값도 허용 |

open-issues P-4는 “모든 설정에 기본값이 있고, 건드리지 않아도 전부 동작한다”로 정한다. InvestorProfile과 구성 요소의 기본값을 테스트로 고정한다. 프로필 필드는 비공개이며 읽기 접근자와 새 값을 반환하는 with_*만 둔다. 입력값을 담은 오류와 Unknown 설정 변형은 없다.

## 몇 시간 단위 기간의 봉인

Horizon은 비공개 필드로 HorizonSelection을 감싼다. 일반 try_new는 Hours를 ConfirmationRequired로 거부한다. 기본값은 WeeksToMonths다. 유일한 별도 생성 경로 from_advanced_confirmation은 AdvancedHoursConfirmation 증거를 요구한다. 증거는 비공개 봉인 타입 필드가 있고 공개 생성자·Default·역직렬화 경로가 없다. 현재 제품 코드 어디에서도 발급하지 않으며 같은 모듈의 테스트에서만 만든다. 증거는 확인된 고급 설정을 나타내는 복사 가능한 값이며 주문용 소비 토큰은 아니다.

SEC-12의 증거 발급 책임은 tasks A-17(위험 설정 변경 유스케이스), U-24(코어가 띄우는 네이티브 확인 창), 고급 허용 설정 O-14와 연결하는 E-09에 있다. 이 후속 구현 전에는 domain 밖에서 몇 시간 단위 Horizon을 생성할 수 없다. 후속 작업은 domain에 검증된 결과를 전달할 신뢰 경계를 설계해야 하며 단순 공개 증거 생성자를 추가해서는 안 된다. 이번 작업은 네이티브 확인의 실제 실행을 증명하지 않는다.

## 종목 유형 목록의 출처와 대응

로컬 고정 사본 docs/toss-openapi-1.2.28.json의 components.schemas.StockInfo.properties.securityType.enum과 components.schemas.ListedStock.properties.securityType.enum은 아래 아홉 값으로 일치한다. 전체 종목 조회의 securityType 쿼리 필터도 일치한다. 거래내역의 assetClass는 별개 분류라 사용하지 않는다.

| 스펙 열거값 | ExcludedSecurityType |
|---|---|
| STOCK | Stock |
| FOREIGN_STOCK | ForeignStock |
| DEPOSITARY_RECEIPT | DepositaryReceipt |
| INFRASTRUCTURE_FUND | InfrastructureFund |
| REIT | Reit |
| ETF | Etf |
| FOREIGN_ETF | ForeignEtf |
| ETN | Etn |
| STOCK_WARRANTS | StockWarrants |

이것은 제외할 수 있는 종목 유형의 초안이며 사용자 확인 대상이다. 우선주는 securityType 열거값이 아니라 isCommonShare의 별도 불린 속성이므로 새 변형을 만들지 않는다. 외부 응답 전체를 표현하는 타입이 아니라 우리 설정의 선택지이므로 Unknown을 추가하지 않는다. 외부 스펙 파싱·새 값 처리는 T-01·T-09 어댑터의 책임이다.

SectorId는 ADR 0022의 InstrumentId 관례에 맞춰 비어 있지 않은 ASCII 표시 문자 1~64바이트의 불투명 식별자다. 실제 존재나 분류 체계를 검증하지 않는다. D-06 SectorAssignment는 이 SectorId를 사용해야 한다. 토스 테마·GICS 등 어느 분류를 선택하는지는 D-06과 데이터 출처 작업에 남긴다.

## 파싱과 경계

ProfileWeight::parse는 NumericInput → Ratio::parse로 D-01·D-02 정규형을 재사용한다. parse_percent도 같은 문법의 숫자만 받는다(20은 20%, % 기호는 입력하지 않는다). 지수·공백·플러스·밑줄·유니코드 숫자를 받지 않고, 선행·후행 0은 허용한다. 256바이트 입력 제한은 기존 NumericInput 경계다.

퍼센트는 먼저 0 초과 100 이하를 검사한 뒤 정규화한 십진 계수의 스케일에 2를 더한다. Decimal의 스케일은 최대 28이라 saturating_add가 포화하지 않는다. checked 생성자가 표현 불가능한 스케일을 거부하므로, 극소 퍼센트를 0으로 반올림하지 않는다. 표시 반올림과 근사 RatioQuotient를 설정 상한에 사용하지 않는다. 일반 Ratio의 음수·1 초과 값은 ProfileWeight 생성에서 거부한다.

## 미해결: 문서 근거 없음, 보수적 초안

- 메인이 정한 경계 초안: 0%는 거부한다. 아무것도 사지 못하게 하는 의도는 제외 설정으로 표현한다. 주의 기준도 양수이며, 주의 기준 ≤ 종목 상한 ≤ 섹터 상한이다. 이 관계와 0 거부는 기존 design의 명시 규칙이 아니라 이번 메인의 보수적 결정이다.
- 제외 목록 자원 경계 초안: 섹터는 256개 이하로 제한하고 257개를 거부한다. 이는 분류 체계의 섹터 수나 투자 정책이 아니라 유한한 설정 입력을 위한 내부 자원 제한이다. 종목 유형은 닫힌 아홉 값과 중복 금지로 최대 9개다. SectorId의 64바이트와 섹터 목록 256개는 제품 문서의 수치가 아니므로 사용자·메인 확인 대상이다.
- 비중 상한 완화와 더 공격적인 위험 성향 변경은 SEC-12의 A-17이 실제 설정 반영 전에 확인한다. D-20은 순수 값 타입이며 값 교체 메서드가 네이티브 확인이나 설정 저장을 실행하지 않는다.

## D-21에 넘길 것

Horizon은 선택지만 담는다. 각 선택지의 실제 시간·거래일·달력일 범위와 H 대응은 이번 작업에 없다. open-issues 1.2는 “(a, b)와 H는 계산을 보이기 위한 예시 값이고 확정값이 아니다”라고 정한다. 0.25·5·40·120거래일을 채택하지 않는다. R-12의 평가 기간 예도 Horizon 범위로 쓰지 않는다. D-21 초안 ADR이 실제 범위, H 대응, 위험 성향별 (a, b)를 정한다. 벽시계 TimeSpan으로 거래일 H를 대신하지 않는다.

## 사용자·메인에게 물을 것

- 스펙의 아홉 종목 유형을 그대로 제외 선택지로 제공할지 확인한다. 우선주만 제외하는 별도 요구가 있으면 후속 범위를 정한다.
- 위 보수적 비중 관계와 제외 목록 자원 경계 초안을 확인한다.

## 검증 방법

기본값, 일반 Hours 거부, 봉인 증거를 받은 Hours 생성, 프로필 교체와 원본 보존, 식별자·목록 상한·중복, 비율·퍼센트 정규형과 극소값, 모순된 상한 조합을 테스트한다. 속성 테스트는 독립 정수 계수 범위·정수 순서·집합 기준을 사용하며 Config::with_cases(4096), 제외 목록은 512회를 명시한다. 외부 crate 컴파일 실험으로 비공개 Horizon·증거 필드와 증거 생성자 부재를 별도로 확인한다.


초기 테스트만 추가한 실행은 unresolved import 오류로 실패했고, 구현 후 domain 테스트 182개와 파일별 커버리지 검사가 통과했다. 외부 rustc 실험은 정상 일반 기간 코드가 종료 0, Horizon 비공개 필드 직접 생성과 확인 증거 직접 생성이 E0423, 증거 Default 호출이 E0599로 실패했다. 실험 파일은 삭제했고 결과는 /tmp/d20-external-compile.log에 남겼다. 이 실험은 영속 compile-fail 회귀 하네스가 아니며 일반 Hours 거부와 확인 경로는 지속 단위 테스트로 검증한다.
