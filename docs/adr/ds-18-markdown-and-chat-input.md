# DS-18 마크다운 표시와 채팅 입력

- 상태: 초안

## 결정

Markdown은 FreeText content, ExternalUrl을 받는 linkAllowed(allow/deny), onOpenLink 세 속성이다. https만 경계에서 파싱하며 수집 목록 판정과 외부 열기는 화면·코어에 남긴다. 원시 HTML은 skipHtml로 생략하고 이미지는 alt(없으면 주소)의 콜백 버튼으로 표시한다. a/href나 img/src는 만들지 않는다. 자동 링크도 같은 판정을 거친다. 승인된 플러그인은 없으므로 CommonMark만 사용한다. GFM 표는 일반 문장, 각주 문법은 CommonMark 참조 정의 규칙을 따른다. 코드 블록과 긴 낱말은 줄바꿈한다.

ChatInput은 onSend(FreeText), status?(ready/sending, 기본 ready)만 받는다. IME isComposing·229·조합 시작 및 종료 직후 Enter를 막는다. Shift+Enter는 기본 줄바꿈이다. 공백만인 입력은 보내지 않고, 전송한 원문은 trim하지 않는다. 전송 뒤 입력을 비우며 sending 동안 입력·버튼을 잠근다. 길이 제한은 문서에 없어 추가하지 않는다.

react-markdown 10.1.0을 정확히 고정했다. 직접 의존의 라이선스는 MIT다. 전이 의존성은 잠금 파일에 기록된다. 새 플러그인은 없다.

보호 파일 변경은 DS-18 공개 진입점, 입력 경계 호출, 제3자 Markdown 콜백과 외부 글 표시 파일 한정 경계, 의존 허용 목록, 화면 첫 사용 전 knip 유예, 줄바꿈 유틸리티와 스냅샷, 생성 해시 등록에 한정한다. 기존 테스트 단언을 약화하지 않는다.

## 인계

- C-03/C-05/C-11: 수집 목록 판정·open_external 연결, sending 상태 소유.
- DS-07/DS-15: 양 테마 이야기의 실제 브라우저 시각 회귀.
- DS-19/DS-20: 이 기준에는 shared/ds/testing과 넘침 검사 구현이 없다. jsdom을 실제 CSS 넘침 증거로 세지 않는다. 자동 넘침 실측 NOT_RUN.
- 문서 정리: docs/security.md가 없어 SEC-10은 design.md를 기준으로 읽었다.
- 세션 프로젝트 신뢰·훅 적용 확인 불가. Git 훅 설치는 실행했다.

## 검증

구현 전 새 테스트가 누락 export로 실패한 기록은 target/DS-18/red.log다. 보안·입력 상태 테스트와 타입/린트 검사를 진행한다. 기존 승인 test-diff 변경 건수는 검증 후 기록한다.

## 손 변형

21개 모두 별도 mktemp 복사본에서 tsc 컴파일 후 ds18-content 테스트를 실제 실행했다. 초기 생존 3개는 전송 중 초안·조합 중 버튼을 검사하도록 보강 후 재실행하여 최종 21개 검출, 생존·컴파일 실패·시간 초과 0이다. 이후 Markdown 콜백 인자를 최소 형태로 명시해 DOM 접근 예외 없이 제3자 원시 타입 경계 파일 한 곳만 허용했다.

기존 고정 목록 테스트 승인 변경: knip.test.mjs expectedProductionAllowlist에 DS-18 의존 1개·파일 3개 추가, local-allowlists.test.mjs의 RAW_TYPE_ALLOWLIST에 Markdown.tsx 1개 추가. 단언 형태 유지.

| 번호 | 파일 | 바꾼 것 | 잡은 테스트 |
|---|---|---|---|
| 1 | ChatInput.tsx | `props.status === "sending" &#124;&#124;` → `false &#124;&#124;` | KILLED ds18-content |
| 2 | ChatInput.tsx | `value.trim() === "" &#124;&#124;` → `false &#124;&#124;` | KILLED ds18-content |
| 3 | ChatInput.tsx | `composition !== "idle"` → `false` | KILLED ds18-content |
| 4 | ChatInput.tsx | `setValue(emptyFreeText);` → `setValue(value);` | KILLED ds18-content |
| 5 | ChatInput.tsx | `props.onSend(value);` → `props.onSend(emptyFreeText);` | KILLED ds18-content |
| 6 | ChatInput.tsx | `props.onSend(value);` → `return;` | KILLED ds18-content |
| 7 | ChatInput.tsx | `disabled={props.status === "sending"}` → `disabled={false}` | KILLED ds18-content |
| 8 | ChatInput.tsx | `props.status !== "sending" && next.kind === "ok"` → `next.kind === "ok"` | KILLED ds18-content |
| 9 | ChatInput.tsx | `setComposition("composing");` → `setComposition("idle");` | KILLED ds18-content |
| 10 | ChatInput.tsx | `setComposition("ending");` → `setComposition("idle");` | KILLED ds18-content |
| 11 | ChatInput.tsx | `if (composition === "ending") setComposition("idle");` → `if (composition === "composing") setComposition("idle");` | KILLED ds18-content |
| 12 | input.ts | `event.key !== "Enter" &#124;&#124; event.shiftKey` → `event.key !== "Enter"` | KILLED ds18-content |
| 13 | input.ts | `event.key !== "Enter" &#124;&#124; event.shiftKey` → `event.shiftKey` | KILLED ds18-content |
| 14 | input.ts | `event.nativeEvent.isComposing &#124;&#124;     legacyComposition(event.nativeEvent) === "composing" &#124;&#124;     composition !== "idle"   )     return "ignore"` → `legacyComposition(event.nativeEvent) === "composing" &#124;&#124;     composition !== "idle"   )     return "ignore"` | KILLED ds18-content |
| 15 | input.ts | `legacyComposition(event.nativeEvent) === "composing" &#124;&#124;     composition !== "idle"   )     return "ignore"` → `composition !== "idle"   )     return "ignore"` | KILLED ds18-content |
| 16 | Markdown.tsx | `skipHtml` → `skipHtml={false}` | KILLED ds18-content |
| 17 | Markdown.tsx | `props.linkAllowed(url.value) === "allow"` → `props.linkAllowed(url.value) === "deny"` | KILLED ds18-content |
| 18 | Markdown.tsx | `props.onOpenLink(url.value);` → `return;` | KILLED ds18-content |
| 19 | Markdown.tsx | `parseExternalUrl(src)` → `parseExternalUrl(undefined)` | KILLED ds18-content |
| 20 | Markdown.tsx | `<pre className="chat-content">{children}</pre>` → `<span>{children}</span>` | KILLED ds18-content |
| 21 | brand.ts | `/^https$/u` → `/^(https&#124;http)$/u` | KILLED ds18-content |

변경분 변이: `just mutants-diff e2b940598678ecc91d49267a3d4c02ccef4c819c` 종료 0, `mutants: 위반 없음`. 이 작업에 Rust 변경이 없어 실행된 Rust 변이를 DS-18의 검출 증거로 세지 않는다. application/domain/order-execution은 명령이 주간 전체 대상으로 안내했다.

기존 고정 스냅샷 승인 변경: ds-generation.test.mjs의 CSS entry 기대 문자열에 DS-18 줄바꿈 유틸리티 9개 선언을 추가했다. 기존 선언과 단언은 유지했다. 승인 test-diff 대상은 총 3개 파일이다.

기존 승인 테스트 추가 갱신: ds-screen-bootstrap.test.mjs의 input 공개 함수 목록과 반환 목록에 chatEventValue·chatCommit을 추가하고 lib 스텁에 parseFreeText·FreeText 재수출을 더했다. 기존 단언 유지. 승인 test-diff 파일은 knip/local-allowlists/ds-generation/ds-screen-bootstrap 4개다. raw 허용목록 삭제 증거는 새 harness/eslint-violations/src/shared/ds/Markdown.tsx와 cases.json의 통과 항목이다.
