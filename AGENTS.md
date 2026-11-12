# Stock Copilot 저장소 지침

토스증권 계좌를 연결해 구독 AI가 종목별 확률을 추정하면, 앱의 코드가 그 확률로 매수·매도 신호를 띄우고 사람이 승인하는 개인용 macOS 데스크톱 앱이다.

## 먼저 읽을 문서

| 문서 | 내용 |
|---|---|
| [docs/open-issues.md](docs/open-issues.md) | 사용자가 확정한 결정과 실측 항목. 다른 문서와 부딪히면 이것이 앞선다 |
| [docs/design.md](docs/design.md) | 제품 설계와 비기능 요구사항 (SEC, ORD, ACC, PERF, API, REL, AI, UX, MNT) |
| [docs/architecture.md](docs/architecture.md) | 시스템·코드 구조, 의존 규칙, 주문 안전 구조 |
| [docs/harness.md](docs/harness.md) | 강제 규칙과 보호 파일 |
| [docs/workflow.md](docs/workflow.md) | 구현 → 적대적 리뷰 → 비판 → 평가 → 수정 절차 |
| [docs/tasks.md](docs/tasks.md) | 작업 단위 |
| [docs/adr/](docs/adr/) | 확정한 결정과 버린 대안 |
| [docs/research.md](docs/research.md) | 리서치 목록과 프롬프트 |

## 반드시 지킬 것

1. 구현은 항상 새 Opus 5.5 서브에이전트(`implementer`)가 한다. 메인 에이전트는 코드를 직접 쓰지 않는다.
2. 구현이 끝나면 항상 새 Opus 5.5 서브에이전트로 적대적 리뷰(`adversarial-reviewer`)와 비판(`review-critic`)을 거치고, 메인 에이전트가 평가해 수정 여부를 정한다. 리뷰어와 비판자에게 구현자의 보고서를 주지 않는다.
3. 구현자 보고의 첫 줄은 "완료" 또는 "막힘: <한 줄 이유>"다.
4. `just check`가 통과해야 작업이 끝난 것이다. 검사를 끄거나 테스트를 약하게 만들어 통과시키지 않는다.
5. 보호 파일(docs/harness.md 7.1)과 기존 테스트를 바꾸지 않는다. 바꿔야 하면 멈추고 보고한다. 이런 변경은 사용자 서명 커밋으로만 들어간다.
6. 주석을 쓰지 않는다. 검사 끄기는 규칙을 지킬 방법이 없을 때만, 한 줄·규칙 하나·이유를 적는 형태로만 쓴다(docs/harness.md 2.1). 끈 곳은 보고에 적는다.
7. AI는 확률과 근거만 낸다. 임계값 비교, 방향, 수량, 가격은 코드(Rust `domain`)가 정한다. 금액, 비중, 확률 계산도 `domain`에서만 한다. TypeScript는 표시만 한다.
8. 사람 승인 없이 주문이 나가는 경로를 만들지 않는다. AI와 MCP 쪽 코드는 주문 전송 포트에 닿지 않는다.
9. 단순함 원칙(docs/open-issues.md 0장 P-1~P-10)을 지킨다. 사용자 절차와 설정을 늘리지 않고, 내부 장치의 이름과 상태를 화면에 드러내지 않는다.
10. 토스 키와 실제 계좌 데이터를 다루지 않는다. 실제 키가 필요한 일은 사용자가 한다.
11. 구조에 영향을 주는 선택은 `docs/adr/`에 기록한다.
12. 원시 타입을 그대로 쓰지 않는다. Rust는 newtype, TypeScript는 브랜드 타입만 쓴다. 불린도 쓰지 않고 열거형(TS는 리터럴 유니온)으로 쓴다. 화면 문구는 `messages.ts`에만 둔다. `string`, `number`, `boolean`, `String`, `u64`, `bool`, `Decimal` 같은 타입은 경계 어댑터와 허용 목록 파일에만 있다 (docs/harness.md 3.4, 4.2).

## 언어

문서와 사용자 화면은 한국어다. 코드 식별자는 영어다.
