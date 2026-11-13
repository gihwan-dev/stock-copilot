# D-20 초안: 투자 성향의 선택지와 기본값

- 상태: 초안, 종목 유형 목록과 보수적 경계는 사용자 확인 대상
- 최초 기준: d4ab5a2287b04757ca63066afa8e57c8bd232622
- 적대적 리뷰 수정 기준: dac5295a8df42be25726b1eed2873ee4fcdae8ff
- 근거: design 6.2·8장·SEC-12, open-issues 1.2·1.3·P-4, tasks D-20, ADR 0022·0025·0028·0029, 이번 메인 결정

## 값 집합과 기본값

| 구성 요소 | 닫힌 값 집합 또는 범위 | 기본값과 근거 |
|---|---|---|
| HorizonSelection | FewDays(며칠), WeeksToMonths(몇 주~몇 달), HalfYearOrMore(반년 이상), Hours(몇 시간 단위) | WeeksToMonths. design 6.2의 기간 기본값 “몇 주~몇 달” |
| HoursPermission | Disabled(허용 안 함), Enabled(허용) | Disabled. design 6.2·8장과 SEC-12의 고급 설정 “몇 시간 단위 기간 허용” |
| RiskAppetite | Conservative(보수), Neutral(중립), Aggressive(공격) | Neutral. design 6.2 “보수 / 중립 / 공격”, 기본값 “중립” |
| Exclusions | SectorId 목록과 ExcludedSecurityType 목록. 중복을 거부하고 입력 순서를 보존 | 빈 두 목록. design 6.2 “종목 유형, 섹터”, 기본값 “없음”. 개별 종목·시장 제외를 추가하지 않음 |
| ProfileWeight | Ratio를 감싼 전용 타입. 0 초과 1 이하 | 종목 상한 0.2, 주의 기준 0.1, 섹터 상한 0.4. design 6.2 “종목 20%(10%부터 주의), 섹터 40%” |
| WeightCaps | 주의 기준 ≤ 종목 상한 ≤ 섹터 상한 ≤ 100% | 위 세 기본값. 서로 같은 값도 허용 |

open-issues P-4는 “모든 설정에 기본값이 있고, 건드리지 않아도 전부 동작한다”로 정한다. InvestorProfile과 구성 요소의 기본값을 테스트로 고정한다. 프로필 필드는 비공개이며 읽기 접근자와 새 값을 반환하는 with_*만 둔다. 입력값을 담은 오류와 Unknown 설정 변형은 없다.

## 허용 설정과 기간 선택의 분리

적대적 리뷰 후 기간 선택과 고급 허용 설정을 분리했다. Horizon은 순수 선택지이며 Horizon::new로 Hours도 만들 수 있지만, 이것만으로 프로필을 만들 수는 없다. InvestorProfile은 HoursPermission을 별도로 보유하고 Hours를 선택한 경우 Enabled여야 한다. 기본값은 WeeksToMonths와 Disabled다. 일반 try_new, with_horizon, with_hours_permission은 이 정합을 검증한다. 위험 성향·제외·비중의 교체는 이 두 값을 바꾸지 않는다.

HoursPermission의 필드는 비공개다. 평상시 허용 설정을 만드는 공개 경로는 Default(Disabled)와 from_advanced_confirmation(Enabled)뿐이다. Enabled 전환은 AdvancedHoursConfirmation을 인자로 받는다. 증거에는 비공개 봉인 타입 필드가 있고 공개 생성자·Default·역직렬화 경로가 없다. 현재 제품 코드에서는 발급하지 않으며 같은 모듈의 테스트에서만 만든다. 허용 설정은 확인 후 보존하는 설정 값이며 기간을 몇 시간 단위로 선택했다는 뜻이 아니다.

몇 시간 단위가 선택된 상태에서 허용을 끄려 할 때 기간을 자동 변경하는 규칙은 design에 없다. 따라서 HoursNotAllowed로 거부하고 원래 프로필을 그대로 둔다. 먼저 다른 기간으로 바꾼 뒤 허용을 끌 수 있다. 새 확인 창을 요구하거나 기본 기간으로 조용히 떨어지는 동작은 추가하지 않는다.

증거 발급 책임은 tasks A-17(위험 설정 변경 유스케이스), U-24(코어 네이티브 확인 창), O-14와 E-09에 있다. 확인 증거가 Copy이고 특정 확인 요청에 묶이지 않는 점은 발급 쪽 A-17·U-24가 설계할 것이다. 이번 값 타입은 실제 네이티브 확인의 실행이나 증거의 일회성·재사용 방지를 증명하지 않는다.

## 저장된 상태의 복원과 호출 경계

SavedInvestorProfile은 저장 경계에서 읽은 미검증 입력 묶음이다. 기간 선택, 허용 상태, 위험 성향, 섹터·종목 유형 목록, 일반 Ratio인 종목 상한·주의 기준·섹터 상한을 담는다. 입력 묶음 자체가 유효한 프로필이라는 뜻은 아니다. saved_state와 개별 접근자로 저장에 필요한 모든 정보를 읽을 수 있다.

InvestorProfile::restore_from_saved_state는 확인 증거를 요구하지 않는다. 각 Ratio를 ProfileWeight::try_new로 재검증하고 WeightCaps::try_new로 관계를 검사한다. Exclusions::try_new로 목록 상한·중복을 재검증한 뒤 일반 프로필 생성자로 기간·허용 정합까지 다시 검사한다. 오류에는 입력값이 없다. 섹터 ID는 이미 봉인된 SectorId 타입으로 받으므로 문자열 형식은 SectorId 생성자가 검증한다. HoursPermission의 crate 내부 복원 함수는 이 복원 경로에서만 쓰며 domain 밖에 공개하지 않는다.

**S 묶음의 프로필 저장 작업 S-10(설정 저장소)만 이 복원 경로를 호출한다. 다른 crate의 호출을 막는 구조 검사는 하네스 작업으로 넘긴다.** 공개 복원 함수의 호출자를 domain 타입만으로 구분할 수 없다는 인계이며 D-16의 사용자 확인 함수와 같은 종류의 호출 경계 계약이다. WebView 설정 변경이나 A-17의 일반 변경에서 복원 함수를 호출해서 확인을 우회해서는 안 된다. 이번 작업에는 그 구조 검사를 추가하지 않는다.

공개 생성 경로는 다음과 같다.

| 대상 | 공개 경로 | 검증·의미 |
|---|---|---|
| Horizon | new(HorizonSelection), Default | 선택지만 담는다. Default는 WeeksToMonths |
| HoursPermission | Default, from_advanced_confirmation | Disabled 또는 봉인 증거를 받은 Enabled |
| InvestorProfile | Default, try_new | 기본값 또는 검증된 구성 값과 허용·기간 정합 검사 |
| 저장된 InvestorProfile | restore_from_saved_state(SavedInvestorProfile) | S-10 전용 계약. 확인 증거 없이 모든 입력 불변 조건을 재검증 |
| InvestorProfile 교체 | with_horizon, with_hours_permission | 정합하지 않으면 오류. 원본 유지 |
| 그 밖의 프로필 교체 | with_risk_appetite, with_exclusions, with_weight_caps | 유효한 값 교체. SEC-12의 실제 변경 확인은 A-17 |

## A-17이 쓸 변경 판정

SEC-12는 “위험 성향을 더 공격적으로 바꾸기”, “비중 상한 완화”를 위험 설정 변경으로 정한다. 값만으로 정해지는 단순 비교이므로 D-20에 둔다. RiskAppetite는 Conservative < Neutral < Aggressive의 닫힌 순서를 제공한다. WeightCaps::change_from(previous)는 종목 상한 또는 섹터 상한 하나라도 커졌으면 Relaxed, 아니면 NotRelaxed다. 다른 상한이 동시에 작아져도 증가한 상한의 위험을 상쇄했다고 보지 않는다. 주의 기준은 신호·수량을 제한하는 상한이 아니므로 주의 기준만 증가한 것은 이 함수의 완화 판정에 포함하지 않는다. 위험 성향별 계수나 정량 위험 점수는 만들지 않는다. A-17이 이 순서와 판정을 써서 네이티브 확인을 거친 뒤 실제 설정을 반영한다.

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

이것은 제외할 수 있는 종목 유형의 초안이며 사용자 확인 대상이다. 우선주는 securityType 열거값이 아니라 isCommonShare의 별도 불린 속성이므로 새 변형을 만들지 않는다. 외부 응답 전체를 표현하는 타입이 아니라 우리 설정의 선택지이므로 Unknown을 추가하지 않는다. 외부 스펙 파싱은 T-01·T-09 어댑터의 책임이다. 닫힌 열거형 밖의 새로운 securityType은 현재 어떤 제외 선택지에도 대응하지 않는다. 따라서 모르는 유형의 종목은 신호·주문 대상에서 차단하는 보수적 규칙을 D-24(신호 판정)와 T-09(종목 마스터 어댑터)에 인계한다. 이 제외 판정 함수의 Included를 모르는 외부 유형의 거래 허용으로 해석해서는 안 된다.

SectorId는 ADR 0022의 InstrumentId 관례에 맞춰 비어 있지 않은 ASCII 표시 문자 1~64바이트의 불투명 식별자다. 실제 존재나 분류 체계를 검증하지 않는다. 대소문자를 정규화하지 않으므로 Energy와 energy는 서로 다른 ID이며 함께 목록에 들어갈 수 있다. ADR 0022의 정규화하지 않는 관례를 따르고 실제 분류의 정규화는 D-06이 정한다. Exclusions::sector_decision과 security_type_decision은 Excluded/Included를 반환하여 D-12·D-24가 목록을 직접 탐색하지 않게 한다. D-06 SectorAssignment는 이 SectorId를 사용해야 한다. 토스 테마·GICS 등 어느 분류를 선택하는지는 D-06과 데이터 출처 작업에 남긴다.

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

기본값, 허용 설정을 바꿔도 기간이 바뀌지 않음, 허용 없는 Hours의 일반 생성·수정·복원 거부, Hours인 상태에서 허용 끄기 거부와 원본 유지, 접근자로 읽은 Hours 프로필의 확인 없는 저장 왕복을 검증한다. 저장된 세 비중 각각의 음수·0·1 초과와 상한 관계, 제외 목록 중복·크기 상한을 재검증한다. 위험 성향의 3×3 순서와 각 상한의 증가·감소·혼합 변경, 주의 기준만 바뀐 경우, 대소문자를 구별하는 제외 판정을 테스트한다.

Config::with_cases(4096)으로 복원의 세 비율 범위를 독립 정수 범위·순서와 대조하고 비중 완화를 정수 비교 기준과 대조한다. 기존 비율·퍼센트 및 제외 속성 검사는 유지한다. 손 변형은 허용·기간 정합 검사 제거와 복원 경로 비중 검사 우회를 각각 주입하여 해당 회귀 테스트가 실패함을 확인한다. 외부 crate 실험은 비공개 허용 설정·확인 증거를 직접 만들 수 없음과 공개 저장 복원 경로를 확인한다. 실제 저장소 호출 제한과 네이티브 확인의 실행은 위 후속 작업의 검증 범위다.

회귀 테스트를 먼저 추가한 실행은 복원·허용·판정 API가 없어 컴파일 실패했다. 구현 후 domain 191개 테스트가 통과했고 네 수정 모듈의 줄 커버리지는 각각 100%였다. 허용·기간 정합 가드를 제거한 손 변형과 복원 경로의 종목 상한 검사를 기본값으로 대체한 손 변형은 각각 해당 회귀 테스트에서 종료 100으로 실패했다. 원본을 복구한 뒤 전체 domain 테스트 191개가 다시 통과했다. 외부 crate 실험은 저장 왕복 코드의 컴파일·실행 성공, 비공개 허용 설정·확인 증거 직접 생성의 E0423, 확인 증거 Default 호출의 E0599를 확인했다. 임시 실험 파일은 삭제했다.
