# DS-21 동작 슬롯 계약

- 상태: 초안

## 결정

Card.actions와 EmptyState.action은 StatusBar.action에서 추출한 Action을 공유한다. UiText label, 값 없는 onPress 콜백, enabled/disabled만 받는다. Dialog.actions는 confirm과 cancel 두 이름 붙은 자리이며 각각 같은 Action이다. 배열과 임의 요소는 받지 않는다. 각 컴포넌트가 Button을 직접 그린다.

화면이 모양을 고르지 않는다는 최상위 기준에 따라 Card·EmptyState·StatusBar는 기존 secondary 변형을 내부에서 고정한다. Dialog는 기존 primary 변형이 있으므로 confirm 자리를 primary, cancel 자리를 secondary로 고정한다. 화면은 변형을 고르지 않는다. 위험 전용 변형을 추가하지 않는 이 선택은 사용자에게 승인받았다. StatusBar의 실제 계약에는 variant가 없으며 Button에는 primary/secondary/ghost만 있다. 위험 전용 변형을 새로 만들지 않고 기존 계약을 재사용한다. 이 선택은 위험 동작을 식별하는 새 제품 의미를 추가하지 않는다. 주문 승인 창은 architecture 5.1에 따라 네이티브 창이며 이 슬롯으로 주문 승인 기능을 추가하지 않는다.

Card·EmptyState·StatusBar는 기존 sm 크기, Dialog는 기본 크기다. Dialog 슬롯 버튼은 type=button이며 Enter가 폼을 제출하거나 확인 콜백을 자동 실행하지 않는다. 취소 자리도 이름만으로 닫지 않고 명시된 콜백을 실행한다. 초점·Escape·배경·닫기 동작은 기존 구현을 유지한다.

## 검증

기존 ds04-components의 makeAction 자료를 JSX에서 객체로 바꾼다. 기존 단언은 그대로다. 이야기 호출부 네 곳(Card 두 곳, EmptyState 한 곳, Dialog 한 곳)과 기존 테스트 자료 한 곳을 이관한다. 화면 사용처는 없다.

새 타입 검사는 JSX·배열·임의 자리·원시 라벨·불린·variant 주입을 거부한다. 기존 린트는 화면의 as any와 JSX 스프레드 우회를 거부하므로 규칙을 추가하지 않는다. 구조적 타입으로 추가 필드를 가진 변수가 전달될 수는 있으나 렌더러는 label/onPress/disabled만 읽으며 DOM으로 펼치지 않는다.

## 인계

- DS-20: 이 기준 트리에는 shared/ds/testing과 넘침 검사 구현이 없다. 넘침 검사는 NOT_RUN이다. jsdom 이야기 렌더를 실제 CSS 넘침 검증으로 세지 않는다.
- DS-15·DS-16: 실제 양 테마 시각 회귀와 WKWebView 접근성은 해당 작업에서 실측한다.

## 손 변형

mktemp 복사본에서 각 변형을 TypeScript로 컴파일한 뒤 ds21-actions 테스트를 실행했다. 최초 실행에서 취소 disabled 무시 변형이 생존하여 새 테스트를 보강했다. 재개 후 자리별 primary/secondary 고정과 서로 다른 confirm/cancel 비활성 상태를 검증하도록 보강한 최종 자료로 29개를 다시 실행했다. 전부 컴파일됐으며 전부 검출됐다.

| 번호 | 대상 | 바꾼 것 | 결과 |
|---|---|---|---|
| 1 | Card | `props.actions.onPress → (): void => { return; }` | KILLED (ds21-actions) |
| 2 | Card | `props.actions.label → text("app.title", "ko")` | KILLED (ds21-actions) |
| 3 | Card | `props.actions.disabled ?? "enabled" → "disabled"` | KILLED (ds21-actions) |
| 4 | Card | `props.actions.disabled ?? "enabled" → "enabled"` | KILLED (ds21-actions) |
| 5 | Card | `props.actions.disabled ?? "enabled" → props.actions.disabled === "disabled" ? "enabled" : "disabled"` | KILLED (ds21-actions) |
| 6 | EmptyState | `props.action.onPress → (): void => { return; }` | KILLED (ds21-actions) |
| 7 | EmptyState | `props.action.label → text("app.title", "ko")` | KILLED (ds21-actions) |
| 8 | EmptyState | `props.action.disabled ?? "enabled" → "disabled"` | KILLED (ds21-actions) |
| 9 | EmptyState | `props.action.disabled ?? "enabled" → "enabled"` | KILLED (ds21-actions) |
| 10 | EmptyState | `props.action.disabled ?? "enabled" → props.action.disabled === "disabled" ? "enabled" : "disabled"` | KILLED (ds21-actions) |
| 11 | StatusBar | `action.onPress → (): void => { return; }` | KILLED (ds21-actions) |
| 12 | StatusBar | `action.label → text("app.title", "ko")` | KILLED (ds21-actions) |
| 13 | StatusBar | `action.disabled ?? "enabled" → "disabled"` | KILLED (ds21-actions) |
| 14 | StatusBar | `action.disabled ?? "enabled" → "enabled"` | KILLED (ds21-actions) |
| 15 | StatusBar | `action.disabled ?? "enabled" → action.disabled === "disabled" ? "enabled" : "disabled"` | KILLED (ds21-actions) |
| 16 | Dialog | `props.actions.confirm.onPress → (): void => { return; }` | KILLED (ds21-actions) |
| 17 | Dialog | `props.actions.confirm.label → text("app.title", "ko")` | KILLED (ds21-actions) |
| 18 | Dialog | `props.actions.confirm.disabled ?? "enabled" → "disabled"` | KILLED (ds21-actions) |
| 19 | Dialog | `props.actions.confirm.disabled ?? "enabled" → "enabled"` | KILLED (ds21-actions) |
| 20 | Dialog | `props.actions.confirm.disabled ?? "enabled" → props.actions.confirm.disabled === "disabled" ? "enabled" : "disabled"` | KILLED (ds21-actions) |
| 21 | Dialog | `props.actions.cancel.onPress → (): void => { return; }` | KILLED (ds21-actions) |
| 22 | Dialog | `props.actions.cancel.label → text("app.title", "ko")` | KILLED (ds21-actions) |
| 23 | Dialog | `props.actions.cancel.disabled ?? "enabled" → "disabled"` | KILLED (ds21-actions) |
| 24 | Dialog | `props.actions.cancel.disabled ?? "enabled" → "enabled"` | KILLED (ds21-actions) |
| 25 | Dialog | `props.actions.cancel.disabled ?? "enabled" → props.actions.cancel.disabled === "disabled" ? "enabled" : "disabled"` | KILLED (ds21-actions) |
| 26 | Dialog | `variant="primary" → variant="secondary"` | KILLED (ds21-actions) |
| 27 | Dialog | `variant="secondary" → variant="primary"` | KILLED (ds21-actions) |
| 28 | Dialog | `disabled={props.actions.confirm.disabled ?? "enabled"} → disabled={props.actions.cancel?.disabled ?? "enabled"}` | KILLED (ds21-actions) |
| 29 | Dialog | `disabled={props.actions.cancel.disabled ?? "enabled"} → disabled={props.actions.confirm?.disabled ?? "enabled"}` | KILLED (ds21-actions) |
