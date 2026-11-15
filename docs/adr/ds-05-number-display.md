# DS-05: 코어 표시값과 방향 계약 검증

- 상태: 초안
- 관련 작업: DS-05, D-01, DS-04, DS-10, U-02, DS-16, IPC
- 리뷰 수정 기준: fa7f4ce0f32548cce612636102d43845dd3401d9

## 결정

사용자에게 틀린 숫자가 그럴듯하게 보이면 안 된다. 방향으로 값의 부호를 바꾸지 않는다. 코어가 준 0이 아닌 음수와 up, 양수와 down, 0이 아닌 값과 flat은 코어 계약 위반이며 표시 전체가 unavailable이다. PriceChange는 금액과 비율 어느 한쪽만 잘못되어도 전체를 닫는다. 표시 0이면 숫자의 부호만 제거하고 원래 방향은 유지한다(ADR 0025). 런타임의 잘못된 방향과 필수 방향 누락도 unavailable이다. 닫힌 값 밖의 처리는 unknown 경계의 switch 이후 fallback으로 처리하며 검사 끄기나 default 금지 규칙 변경은 없다.

소수 비율 Ratio·Probability와 표시 전용 PercentDisplay는 서로 다른 비공개 symbol 키를 가진 불투명 객체다. parsePercentDisplay는 코어가 퍼센트 단위 환산·반올림·자릿수 확정을 끝낸 십진수 문자열을 받는다. Percent, Probability, PriceChange.rate는 PercentDisplay만 받는다. 소수 비율 객체를 전달하면 타입 오류이며, 런타임의 잘못된 타입도 표시 불가로 닫힌다. 확률은 디자인 명세대로 1~99의 정수 문자열만 표시한다. 0·100·소수·음수는 unavailable이다. 일반 Percent의 자릿수는 코어가 보낸 그대로 보존한다.

공개 컴포넌트는 여섯 개를 유지한다. Money는 Money | Price를 받아 현재가·평단가도 표시한다. Money와 Percent의 선택 direction은 부호·색·기호·접근성 이름을 함께 표시하고, 생략하면 중립 표시다. 수량·확률에 다른 모양 prop은 추가하지 않는다. Money의 notation 기본값은 full이다. compact는 원화 정수만 만·억·조로 네 자리씩 분리한다. 조보다 큰 부분은 조 개수에 세 자리 쉼표를 붙인다. USD 또는 소수점이 있는 문자열은 소수부가 모두 0이어도 full로 표시한다. 음수 USD 순서는 −$182.34다.

형식화는 문자열 연산뿐이다. Intl·Number·parseFloat·parseInt·프런트 반올림을 금지한다. 정규식 뒤의 중복 lookahead와 trim 검사를 제거했다. 멀티라인 없는 JavaScript의 `^…$` 정규형 검사는 앞뒤 공백과 마지막 줄바꿈을 거부한다. Node 실행으로 `1\n`, `1\r\n`, 앞뒤 공백의 거부를 확인했다. 경계 파서는 먼저 입력이 문자열인지 검사하고 숫자·null 등을 거부한다. 이전 ADR의 마지막 줄바꿈 설명은 잘못되어 이 내용으로 교정했다. 정상 값의 끝자리 0은 보존한다.

format.ts의 함수와 타입은 shared/lib/index.ts에서 재수출하지 않는다. 가져올 수 있는 제품 파일은 shared/ds/Numbers.tsx 하나다. 모듈 경계와 실제 선언을 추적하는 별칭·재수출 검사를 함께 둔다. readDecimalForNumberDisplay는 money.ts·format.ts만 쓴다. FormattedNumber의 JSX 허용은 shared/ds 경로에만 적용한다. 화면은 숫자 DS 컴포넌트만 조립한다.

unavailable은 경고 토큰 text-alert의 —와 messages.ts의 UiText 접근성 이름 “값을 표시할 수 없음”을 가진다. 숫자·방향 기호는 tabular-nums·whitespace-nowrap를 유지한다. 숫자를 자르거나 말줄임하지 않는다. DirectionBadge 기호는 aria-hidden=true로 숨기고 방향 이름을 읽는다. 정상적인 값 없음이라는 새 공개 prop은 만들지 않았다. 코어의 선택적 데이터 부재는 화면 조립 계약에서 처리하고, 필수 숫자 표시 입력의 누락·형식 오류·계약 위반은 닫힌 unavailable로 처리한다.

## 지적별 처리

| 번호 | 처리 |
|---|---|
| 1 | 부호·방향 불일치는 전체 —, 반올림 0은 원래 방향 유지 |
| 2 | PercentDisplay·파서·독립 키, Ratio·Probability 거부 타입 예제 |
| 3 | 형식화 공개 재수출 제거, Numbers.tsx 전용 경계, DS 전용 FormattedNumber JSX |
| 4 | 확률 1~99 정수만 표시 |
| 5 | Money·Percent 선택 direction, Price는 Money에서 표시 |
| 6 | USD compact는 full로 대체 |
| 7 | architecture 5.3·R-14의 제품 결정과 방향 미결 문구 확정 |
| 8 | 경고색 —·UiText 접근성 이름, 전체 표시 거부 |
| 9 | 조 단위 및 조 이상 쉼표 |
| 10 | 소수점이 있으면 full |
| 11 | 닫힌 값 밖 방향은 fallback unavailable |
| 12 | 중복 lookahead 제거·정규식 설명 수정 |
| 13 | 경계·별칭·접근성·스타일·타입 회귀와 새 손 변형 실제 실행 |

## 공개 계약

| 컴포넌트 | prop |
|---|---|
| Money | value: Money 또는 Price, notation?: full/compact, direction?: up/down/flat |
| Percent | value: PercentDisplay, direction?: up/down/flat |
| Quantity | value: Quantity |
| Probability | value: PercentDisplay |
| PriceChange | amount: Money, rate: PercentDisplay, direction: up/down/flat |
| DirectionBadge | direction: up/down/flat |

## 예제

| 입력 | 출력 |
|---|---|
| KRW 84312500 | 84,312,500원 |
| USD -182.34 | −$182.34 |
| KRW 1234000 / up | ▲ +1,234,000원 |
| PercentDisplay 5.9 / up | ▲ +5.9% |
| Price USD 182.34 | $182.34 |
| 금액 -1700·비율 -2.43 / up | — |
| 금액 -5 / flat | — |
| 금액 -0.00·비율 0.000 / down | ▼ 0.00원 (0.000%) |
| Probability 64 | 64% |
| Probability 0·100·64.5·1.0 | — |
| 소수 비율 Ratio 0.1516을 Percent에 전달 | 타입 오류, 런타임 손상 입력도 — |
| KRW compact 1000000000000 | 1조원 |
| KRW compact 1234567890123 | 1조 2,345억 6,789만 123원 |
| KRW compact 1234567890123456 | 1,234조 5,678억 9,012만 3,456원 |
| USD compact 84312500 | $84,312,500 |
| KRW compact 10000.00 | 10,000.00원 |
| 잘못된 방향·형식·통화·입력 객체 | — |

## 보호 경계와 기대값 변경

money.ts에는 PercentDisplay와 파서를 추가하고 표시 읽기 함수는 Money·Price·Quantity·PercentDisplay만 읽는다. lib/index.ts는 표시 전용 파서·타입만 공개하고 형식화 API·FormattedNumber·Direction 재수출을 제거한다. 브랜드 생성과 읽기 권한을 화면에 새로 열지 않는다. Numbers.tsx만 format.ts를 가져오도록 boundaries.mjs에 정확한 허용·거부 정책을 등록했다. no-ui-text는 경로를 확인해 DS에서만 FormattedNumber를 허용한다. number-display-boundary는 프로젝트 상대 경로와 TypeScript의 실제 선언·별칭을 확인한다. 다른 파일의 동명 함수는 권한을 얻거나 잘못 거부되지 않는다.

중앙 cases.json은 formatter 재수출의 정확한 두 오류(boundaries와 전용 규칙)를 추가하고, Numbers.tsx의 합법 소비 예제를 추가했다. 이전 회차의 DS-05 테스트에서 부호를 덮어쓰던 기대값, 확률 소수 허용, 소수 compact 기대값을 이번 결정으로 교체했다. 이번 작업에서 추가한 회귀 테스트는 금액·비율·통화를 서로 다른 값으로 검증한다. 기존 하네스의 기계적 기대값 변경은 승인 범위에서만 수행하고 전후를 최종 보고에 남긴다.

새 외부 의존성·검사 끄기는 없다. tasks.md·progress.md는 직접 수정하지 않는다. 생성물은 저장소 생성기로만 갱신한다. 프로젝트·Codex 훅 신뢰 저장 상태는 이전 회차와 같이 미확인이고, 실제 Git 커밋 훅과 전체 검사 출력으로 검증한다.

## 인계

- U-02·IPC: DTO가 이미 퍼센트 단위 환산·반올림된 표시 문자열을 보내고 parsePercentDisplay로 파싱한다. 원시 소수 비율을 표시 파서에 다시 포장하지 않는다. 일반 Probability의 소수 비율과 PercentDisplay의 단위는 계약에서 분리한다.
- 메인·DS-16: tasks.md의 “Intl 결과를 기록”을 “실제 WKWebView에서 문자열 형식화 결과, U+2212, 끝자리 0, 고정폭 숫자, 라이트·다크, unavailable 접근성 이름을 기록”으로 바꾼다. tasks.md는 메인이 수정한다.
- DS-07: 정상·방향·오류 상태의 numberComponentStories를 실제 목록과 시각 회귀 환경에 연결한다.
- DS-19·DS-20: 숫자를 담는 표·카드의 가로 스크롤과 실제 창 넘침을 검증한다.

## 손 변형 실행 결과

최종 코드 복사본의 기준 타입 검사와 회귀 8개가 통과한 뒤 손 변형 56개를 실행했다. 56개 모두 TypeScript 컴파일을 통과했고 JavaScript 린트 변형은 Node 문법 검사도 통과했다. 실제 테스트 실패로 56개 모두 검출되어 생존·시간 초과 0개다. 임시 복사본은 삭제했고 실행 로그·변경 내용·컴파일 상태·소스 해시는 target/DS-05-review/mutations.json과 mutation-logs에 보존한다. 이전 회차의 임시 복사본 두 개도 로그만 보존하고 삭제했다.

첫 시도의 스토리 import 누락에 따른 컴파일 실패는 집계하지 않았다. 첫 유효 실행에서 54개 검출·2개 생존을 확인했다. compact 10000 경계 예제를 추가했고, 다른 생존 원인이었던 중복 trim 조건은 삭제했다. 경계 파서의 문자열 타입 검증 변형을 포함한 최종 56개를 모두 다시 실행한 결과가 아래 표다.

| 번호 | 바꾼 것 | 잡은 실제 테스트 | 결과 |
|---|---|---|---|
| 1 | 쉼표 제거 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 2 | 그룹 세 자리→두 자리 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 3 | compact 네 자리 경계 확대 | DS05 review compact is exact KRW integers with trillion grouping | 검출 |
| 4 | 조 상위 분리 위치 이동 | DS05 review compact is exact KRW integers with trillion grouping | 검출 |
| 5 | 조 단위를 억으로 교체 | DS05 review compact is exact KRW integers with trillion grouping | 검출 |
| 6 | 그룹 선행 0 보존 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 7 | 0 그룹 제외 조건 반전 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 8 | 단위 사이 공백 제거 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 9 | USD compact 허용 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 10 | 소수 compact 허용 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 11 | 달러 기호 제거 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 12 | 달러와 부호 순서 역전 | DS05 review standalone directional amounts percentages and prices | 검출 |
| 13 | 음수 기호 하이픈 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 14 | 상승 더하기 제거 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 15 | 음수 0 부호 유지 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 16 | flat 비영 값 허용 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 17 | up 부호 검증 반전 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 18 | down 부호 검증 반전 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 19 | 닫힌 값 밖 방향 허용 | DS05 review rejects inconsistent directions for the whole change | 검출 |
| 20 | 반올림 0 방향 거부 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 21 | 방향 계약 거부 조건 반전 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 22 | 확률 0·100 허용 | DS05 review probability integer boundaries and malformed presentation | 검출 |
| 23 | 확률 소수 허용 | DS05 review probability integer boundaries and malformed presentation | 검출 |
| 24 | 확률 두 자리 정상값 거부 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 25 | 잘못된 십진수 허용 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 26 | 경계 파서 숫자 문자열 강제 변환 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 27 | PriceChange 한쪽 오류 허용 | DS05 review rejects inconsistent directions for the whole change | 검출 |
| 28 | 원 단위 누락 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 29 | 주 단위 누락 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 30 | 퍼센트 단위 교체 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 31 | Money 읽기 누락 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 32 | Price 읽기 누락 | DS05 review standalone directional amounts percentages and prices | 검출 |
| 33 | Quantity 읽기 누락 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 34 | PercentDisplay 읽기 누락 | DS05 document examples and decimal boundaries preserve core digits | 검출 |
| 35 | 알 수 없는 타입을 소수 비율로 표시 | DS05 review probability integer boundaries and malformed presentation | 검출 |
| 36 | Money 기본 notation compact | DS05 review standalone directional amounts percentages and prices | 검출 |
| 37 | 오류 색 중립색 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 38 | 오류 접근성 이름 제거 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 39 | 방향 숫자 nowrap 제거 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 40 | 중립 숫자 tabular 제거 | DS05 review standalone directional amounts percentages and prices | 검출 |
| 41 | 오류 숫자 nowrap 제거 | DS05 direction rejects inconsistent signs and preserves rounded zero direction | 검출 |
| 42 | 방향 배지 aria-hidden 반전 | DS05 review probability integer boundaries and malformed presentation | 검출 |
| 43 | 방향 배지 가격색 고정 | DS05 review probability integer boundaries and malformed presentation | 검출 |
| 44 | PercentDisplay를 Ratio와 같은 키로 합침 | DS05 review probability integer boundaries and malformed presentation | 검출 |
| 45 | FormattedNumber JSX를 화면에 개방 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 46 | parseInt 금지 누락 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 47 | parseFloat 금지 누락 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 48 | Intl 금지 누락 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 49 | Number 금지 누락 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 50 | 별칭 해석 제거 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 51 | 형식화 소비 경로 오등록 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 52 | 화면 읽기 함수 권한 추가 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 53 | 읽기 함수 선언 경로 오등록 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 54 | format 파일 경로를 접미사로 확대 | DS05 reader access, conversion and price color boundaries reject violations | 검출 |
| 55 | 필수 direction 누락 허용 | DS05 review rejects inconsistent directions for the whole change | 검출 |
| 56 | 잘못된 통화 허용 | DS05 review probability integer boundaries and malformed presentation | 검출 |
