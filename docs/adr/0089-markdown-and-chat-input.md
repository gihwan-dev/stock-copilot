# ADR 0089: DS-18 마크다운 표시와 채팅 입력

- 상태: 확정 (2026-11-16)


## 결정

Markdown은 FreeText content, ExternalUrl을 받는 linkAllowed(allow/deny), onOpenLink 세 속성이다. https만 경계에서 파싱하며 수집 목록 판정과 외부 열기는 화면·코어에 남긴다. 원시 HTML은 skipHtml로 생략하고 이미지는 alt(없으면 주소)의 콜백 버튼으로 표시한다. a/href나 img/src는 만들지 않는다. 자동 링크도 같은 판정을 거친다. 승인된 플러그인은 없으므로 CommonMark만 사용한다. GFM 표는 일반 문장, 각주 문법은 CommonMark 참조 정의 규칙을 따른다. 코드 블록과 긴 낱말은 줄바꿈한다.

ChatInput은 onSend(FreeText), status?(ready/sending, 기본 ready)만 받는다. IME isComposing·229·조합 시작 및 종료 직후 Enter를 막는다. Shift+Enter는 기본 줄바꿈이다. 공백만인 입력은 보내지 않고, 전송한 원문은 trim하지 않는다. 전송 뒤 입력을 비우며 sending 동안 입력은 readOnly, 전송 버튼은 disabled로 둔다. 길이 제한은 문서에 없어 추가하지 않는다.

react-markdown 10.1.0을 정확히 고정했다. 직접 의존의 라이선스는 MIT다. 전이 의존성은 잠금 파일에 기록된다. 새 플러그인은 없다.

보호 파일 변경은 DS-18 공개 진입점, 입력 경계 호출, 제3자 Markdown 콜백과 외부 글 표시 파일 한정 경계, 의존 허용 목록, 화면 첫 사용 전 knip 유예, 줄바꿈 유틸리티와 스냅샷, 생성 해시 등록에 한정한다. 기존 테스트 단언을 약화하지 않는다.

## 인계

- C-03/C-05/C-11: 수집 목록 판정·open_external 연결, sending 상태 소유. C-03은 받은 원문의 실패 보존·재시도 UX를 정한다. C-05/C-11은 수집 목록과 코어가 공유할 URL 동일성·정규화 기준을 정한다.
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

## 리뷰 수정 결정

조합 상태가 composing일 때만 버튼을 막고 ending은 즉시 전송한다. ending은 textarea에서 다음 키를 소비할 때 idle로 돌아가며, 이 상태에서 소비하는 Enter만 기본 동작을 취소한다. 조합 확정 중의 native Enter와 Shift+Enter는 기본 동작을 유지한다. 실제 macOS IME 이벤트 순서는 C-03에서 실측한다.

Markdown은 두 개의 내부 Context로 렌더 정책과 링크 자손 여부를 전달한다. 링크 자손 이미지는 글자만 표시하여 바깥 버튼 하나만 존재한다. 단독 이미지는 기존 허용 판정·콜백 버튼을 유지한다. DOM 조상 검사·이벤트 버블링 차단·플러그인·새 공개 prop은 쓰지 않는다. 이미지 버튼에도 같은 focus-visible 토큰을 적용한다.

입력은 기존 min/max 높이 토큰을 적용하고 resize를 막는다. sending은 readOnly로 입력 편집을 막아 포커스 가능한 상태를 유지한다. 실패 복구용 새 prop이나 비동기 반환 계약은 만들지 않는다. onSend에 원문을 넘긴 뒤 입력을 비우며, 화면이 받은 원문을 보존한다.

마크다운 부하는 단락 구분 후 100번 반복해 제목과 코드 블록이 각각 100개인지 양 테마에서 검사한다. ChatInput 부하 이야기는 같은 비공개 Editor에 초기 초안을 주어 render만으로 긴 여러 줄 글을 표시한다. 공개 ChatInput의 prop은 늘리지 않는다. jsdom 렌더를 CSS 넘침·시각 회귀로 세지 않는다.

## 리뷰 수정 후 손 변형

수정 후 최종 트리 복사본에서 24개를 각각 tsc로 컴파일하고 ds18-content 회귀를 실제 실행했다. 전부 검출됐고 생존·컴파일 실패·시간 초과는 0이다. 앞선 21개 표는 첫 구현 검증 기록이다.

| 번호 | 대상 | 바꾼 것 | 잡은 테스트 |
|---|---|---|---|
| 1 | ChatInput.tsx | `props.status === "sending" &#124;&#124;` → `false &#124;&#124;` | KILLED: DS18 pending draft blocks send and edits while sending (16.386917ms) |
| 2 | ChatInput.tsx | `value.trim() === "" &#124;&#124;` → `false &#124;&#124;` | KILLED: DS18 chat rejects blanks and sending, sends original text once (63.802625ms) |
| 3 | ChatInput.tsx | `composition === "composing"` → `false` | KILLED: DS18 pending draft blocks send and edits while sending (23.401791ms) |
| 4 | ChatInput.tsx | `setValue(emptyFreeText);` → `setValue(value);` | KILLED: DS18 chat rejects blanks and sending, sends original text once (27.508041ms) |
| 5 | ChatInput.tsx | `props.onSend(value);` → `props.onSend(emptyFreeText);` | KILLED: DS18 chat rejects blanks and sending, sends original text once (39.66675ms) |
| 6 | ChatInput.tsx | `props.onSend(value);` → `return;` | KILLED: DS18 chat rejects blanks and sending, sends original text once (22.999208ms) |
| 7 | ChatInput.tsx | `readOnly={props.status === "sending"}` → `readOnly={false}` | KILLED: DS18 chat rejects blanks and sending, sends original text once (26.638792ms) |
| 8 | ChatInput.tsx | `props.status !== "sending" && next.kind === "ok"` → `next.kind === "ok"` | KILLED: DS18 pending draft blocks send and edits while sending (16.222875ms) |
| 9 | ChatInput.tsx | `setComposition("composing");` → `setComposition("idle");` | KILLED: DS18 IME and Shift Enter never send (35.602292ms) |
| 10 | ChatInput.tsx | `setComposition("ending");` → `setComposition("idle");` | KILLED: DS18 IME and Shift Enter never send (22.266833ms) |
| 11 | ChatInput.tsx | `if (composition === "ending") setComposition("idle");` → `if (composition === "composing") setComposition("idle");` | KILLED: DS18 IME and Shift Enter never send (20.926666ms) |
| 12 | input.ts | `event.key !== "Enter" &#124;&#124; event.shiftKey` → `event.key !== "Enter"` | KILLED: DS18 IME and Shift Enter never send (31.507709ms) |
| 13 | input.ts | `event.key !== "Enter" &#124;&#124; event.shiftKey` → `event.shiftKey` | KILLED: DS18 IME and Shift Enter never send (29.280208ms) |
| 14 | input.ts | `if (composition === "ending") event.preventDefault();` → `if (composition === "ending") return "ignore";` | KILLED: DS18 ending Enter is consumed while Shift Enter remains native (18.566292ms) |
| 15 | Markdown.tsx | `skipHtml` → `skipHtml={false}` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (264.809625ms) |
| 16 | Markdown.tsx | `props.linkAllowed(url.value) === "allow"` → `props.linkAllowed(url.value) === "deny"` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (239.30875ms) |
| 17 | Markdown.tsx | `props.onOpenLink(url.value);` → `return;` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (244.647791ms) |
| 18 | Markdown.tsx | `parseExternalUrl(src)` → `parseExternalUrl(undefined)` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (236.113292ms) |
| 19 | Markdown.tsx | `<pre className="chat-content">{children}</pre>` → `<span>{children}</span>` | KILLED: DS18 code, footnotes and table syntax remain readable (25.063875ms) |
| 20 | brand.ts | `/^https$/u` → `/^(https&#124;http)$/u` | KILLED: DS18 denied and unsafe addresses stay readable without actions (50.21225ms) |
| 21 | Markdown.tsx | `nesting === "outside"` → `nesting === "inside"` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (242.601833ms) |
| 22 | Markdown.tsx | `<LinkContext value="inside">` → `<LinkContext value="outside">` | KILLED: DS18 image inside a link has one interactive target (16.112959ms) |
| 23 | ChatInput.tsx | `chat-content chat-input` → `chat-content` | KILLED: DS18 stories render both themes with mixed markdown and long draft (114.638917ms) |
| 24 | ChatInput.tsx | `composition === "composing"` → `composition !== "idle"` | KILLED: DS18 completed composition permits one button send (15.850583ms) |

최종 변경분 변이 명령도 종료 0과 `mutants: 위반 없음`을 반환했다. Rust 변이 대상은 없으므로 프런트엔드 변이 검출로 세지 않는다. 좁은 회귀는 ds18-content 11/11, ds-generation 4/4, tsc·eslint·ds-style·review-records 통과다. 기존 고정 CSS 기대값에 chat-input의 min-height/max-height/resize 정확 선언을 추가했고 이전 선언·단언은 유지했다.

## 회차 2 수정 결정

마크다운의 목록 마커·들여쓰기, 제목 계층, 문단 간격, 인용·코드를 확정 토큰으로 복구한다. 순서 목록의 시작 번호는 마크다운 파서가 준 start를 보존한다. HTML 태그 존재만으로 시각 구조를 검증하지 않고, 실제 Tailwind 생성 CSS의 list-style·들여쓰기·문단 간격·제목 크기/굵기와 DOM 렌더 매핑을 함께 확인한다.

모든 마크다운 렌더러와 components 맵을 모듈 수준으로 고정한다. 스트리밍 문단 추가 전후 동일한 링크 버튼·코드 블록이 같은 DOM 노드이며 초점을 유지하는지 검사한다. 읽을 내용이 없는 링크는 비대화형 글자로 처리한다. 사용자 문구를 새로 만들지 않으며 빈 Tab 대상을 만들지 않는다.

입력은 blur/focus 및 전송 완료 때 조합 상태를 idle로 바꾼다. 조합 확정 직후의 Enter 보호는 유지한다. 자동 높이 성장은 요구에 없어 추가하지 않는다. 부하 이야기는 비공개 Editor의 초기 상태를 이용한 이야기 배열로 제공하며 공개 진입점은 기존 두 컴포넌트뿐이다.

C-05/C-11 인계 보완: linkAllowed/onOpenLink 주소는 마크다운 파서의 퍼센트 인코딩·엔티티 처리를 거친 문자열이며, 그 뒤 DS URL 파서는 추가 정규화 없이 검증한다. 화면·코어의 목록 저장과 비교 기준을 이 값에 맞춰 일관되게 정한다. DS-16/C-03은 실제 한글 IME에서 조합 중 보내기 클릭·blur/compositionend 순서·최종 전송 원문을 실측한다.

## 회차 2 최종 손 변형

32개 모두 독립 복사본에서 tsc 컴파일 뒤 DOM·생성 CSS 회귀를 실제 실행했다. 최종 32개 검출, 생존·컴파일 실패·시간 초과 0이다. 최초 적용 문자열이 일치하지 않은 25번은 표현을 바로잡고 전체를 다시 실행했다. 32번은 DOM 객체 실패 출력 때문에 최초 45초 제한에 걸렸으며 성공으로 세지 않았다. 같은 노드 동일성 조건을 불린 단언으로 표현한 뒤 32번을 재실행해 실패 단언·정상 종료를 확인했다.

| 번호 | 대상 | 바꾼 것 | 잡은 테스트 |
|---|---|---|---|
| 1 | ChatInput.tsx | `props.status === "sending" &#124;&#124;` → `false &#124;&#124;` | KILLED: DS18 pending draft blocks send and edits while sending (15.363708ms) |
| 2 | ChatInput.tsx | `value.trim() === "" &#124;&#124;` → `false &#124;&#124;` | KILLED: DS18 chat rejects blanks and sending, sends original text once (18.016291ms) |
| 3 | ChatInput.tsx | `composition === "composing"` → `false` | KILLED: DS18 pending draft blocks send and edits while sending (15.285792ms) |
| 4 | ChatInput.tsx | `setValue(emptyFreeText);` → `setValue(value);` | KILLED: DS18 chat rejects blanks and sending, sends original text once (22.09075ms) |
| 5 | ChatInput.tsx | `props.onSend(value);` → `props.onSend(emptyFreeText);` | KILLED: DS18 chat rejects blanks and sending, sends original text once (22.830292ms) |
| 6 | ChatInput.tsx | `props.onSend(value);` → `return;` | KILLED: DS18 chat rejects blanks and sending, sends original text once (21.098334ms) |
| 7 | ChatInput.tsx | `readOnly={props.status === "sending"}` → `readOnly={false}` | KILLED: DS18 chat rejects blanks and sending, sends original text once (28.924208ms) |
| 8 | ChatInput.tsx | `props.status !== "sending" && next.kind === "ok"` → `next.kind === "ok"` | KILLED: DS18 pending draft blocks send and edits while sending (14.778041ms) |
| 9 | ChatInput.tsx | `setComposition("composing");` → `setComposition("idle");` | KILLED: DS18 IME and Shift Enter never send (18.083167ms) |
| 10 | ChatInput.tsx | `setComposition("ending");` → `setComposition("idle");` | KILLED: DS18 IME and Shift Enter never send (19.187375ms) |
| 11 | ChatInput.tsx | `if (composition === "ending") setComposition("idle");` → `if (composition === "composing") setComposition("idle");` | KILLED: DS18 IME and Shift Enter never send (19.373625ms) |
| 12 | input.ts | `event.key !== "Enter" &#124;&#124; event.shiftKey` → `event.key !== "Enter"` | KILLED: DS18 IME and Shift Enter never send (16.98525ms) |
| 13 | input.ts | `event.key !== "Enter" &#124;&#124; event.shiftKey` → `event.shiftKey` | KILLED: DS18 IME and Shift Enter never send (17.4595ms) |
| 14 | input.ts | `if (composition === "ending") event.preventDefault();` → `if (composition === "ending") return "ignore";` | KILLED: DS18 ending Enter is consumed while Shift Enter remains native (14.664041ms) |
| 15 | Markdown.tsx | `skipHtml` → `skipHtml={false}` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (213.947292ms) |
| 16 | Markdown.tsx | `props.linkAllowed(url.value) === "allow"` → `props.linkAllowed(url.value) === "deny"` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (206.451375ms) |
| 17 | Markdown.tsx | `props.onOpenLink(url.value);` → `return;` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (211.808833ms) |
| 18 | Markdown.tsx | `parseExternalUrl(src)` → `parseExternalUrl(undefined)` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (217.932083ms) |
| 19 | brand.ts | `/^https$/u` → `/^(https&#124;http)$/u` | KILLED: DS18 denied and unsafe addresses stay readable without actions (46.99ms) |
| 20 | Markdown.tsx | `nesting === "outside"` → `nesting === "inside"` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (204.653542ms) |
| 21 | Markdown.tsx | `<LinkContext value="inside">` → `<LinkContext value="outside">` | KILLED: DS18 image inside a link has one interactive target (17.250583ms) |
| 22 | ChatInput.tsx | `chat-content chat-input` → `chat-content` | KILLED: DS18 stories render both themes with mixed markdown and long draft (99.01125ms) |
| 23 | ChatInput.tsx | `composition === "composing"` → `composition !== "idle"` | KILLED: DS18 old composition cannot consume Enter after focus or send (15.179084ms) |
| 24 | Markdown.tsx | `<pre className="chat-content my-space-3 rounded-md bg-surface-sunken p-space-3">     {children}   </pre>` → `<span className="chat-content my-space-3 rounded-md bg-surface-sunken p-space-3">     {children}   </span>` | KILLED: DS18 code, footnotes and table syntax remain readable (21.134917ms) |
| 25 | Markdown.tsx | `hasChildren(node?.children) === "present"` → `hasChildren(node?.children) === "empty"` | KILLED: DS18 markdown excludes HTML and remote images, uses callback only (321.85875ms) |
| 26 | ChatInput.tsx | `    setComposition("idle");     setValue(emptyFreeText);` → `    setValue(emptyFreeText);` | KILLED: DS18 old composition cannot consume Enter after focus or send (15.642625ms) |
| 27 | ChatInput.tsx | `useState(props.initialDraft)` → `useState(emptyFreeText)` | KILLED: DS18 stories render both themes with mixed markdown and long draft (125.829167ms) |
| 28 | Markdown.tsx | `<h1 className="text-title-lg my-space-4">` → `<h1 className="text-title-sm my-space-4">` | KILLED: DS18 semantic nodes retain hierarchy and nondefault list start (19.198125ms) |
| 29 | Markdown.tsx | `<ol start={start}` → `<ol start={start === undefined ? undefined : 1}` | KILLED: DS18 semantic nodes retain hierarchy and nondefault list start (18.445625ms) |
| 30 | ds-design-usage.mjs | `["markdown-ordered", "list-style-type", "decimal"]` → `["markdown-ordered", "list-style-type", "none"]` | KILLED: DS18 generated CSS restores readable markdown after preflight (25.482542ms) |
| 31 | ds-design-usage.mjs | `["markdown-bullets", "list-style-type", "disc"]` → `["markdown-bullets", "list-style-type", "none"]` | KILLED: DS18 generated CSS restores readable markdown after preflight (25.447875ms) |
| 32 | Markdown.tsx | `components={components}` → `components={{ ...components, a: (data: LinkContent): ReactElement => <MarkdownLink href={data.href} node={data.node}>{data.children}</MarkdownLink> }}` | KILLED: DS18 streaming preserves link and code nodes and focus (13.590292ms) |

회차 2 좁은 회귀는 DOM·실제 생성 CSS 17/17, 생성 검사 4/4, tsc·eslint·knip 통과다. 변경분 변이도 종료 0이며 DS-18에 실행 Rust 변이는 없다. 추가 라이선스 자료는 직접 1개·전이 74개를 로컬 설치 메타데이터로 확인했다.

## 회차 3 이름 판정 보완

이미지 표시 문자열과 AST의 읽을 내용 판정을 imageLabel/valuePresent 하나로 통일한다. 공백·NBSP·Default Ignorable 문자만 있는 alt는 src로 대체하며, 표시 문자열까지 비면 버튼을 만들지 않는다. 읽을 글자가 있는 원문은 바꾸지 않고, 가시성 검사에서만 Default Ignorable을 제외한다. 따라서 ZWJ 이모지와 읽을 글자가 섞인 링크의 원문도 보존한다.

마지막 소규모 patch는 세 번째 새 Codex 비판자가 원본/수정본 19개 회귀와 181개 독립 조합으로 해결을 검증했다. 네 번째 Opus 리뷰는 최대 세 회차 지시에 따라 실행하지 않았다. 대상 브라우저의 실제 IME·VoiceOver·빠른 연속 Enter와 inline-block 줄 배치는 여전히 실측 인계다. 보호 파일의 사용자 승인과 암호학적 서명은 다르며, 현재 서명 잠금 전 작업 브랜치 결과를 메인이 서명 반영 절차로 이관한다.

## 마지막 patch 손 변형

현재 source 복사본에서 앞선 32개와 새 이름 판정 3개를 합쳐 35개를 각각 tsc 컴파일·실행했고 전부 검출했다. 생존·컴파일 실패·시간 초과 0이다. 새 변형의 실제 실행 결과는 다음과 같다.

| 번호 | 대상 | 바꾼 것 | 잡은 테스트 |
|---|---|---|---|
| 33 | Markdown.tsx | `valuePresent(alt) === "present" ? alt : src` → `alt === undefined &#124;&#124; alt === "" ? src : alt` | KILLED: DS18 blank image alt uses a readable URL in standalone and nested links (13.429125ms) |
| 34 | Markdown.tsx | `.replace(/\p{Default_Ignorable_Code_Point}/gu, "")` → `` | KILLED: DS18 blank image alt uses a readable URL in standalone and nested links (20.924875ms) |
| 35 | Markdown.tsx | `imageLabel(node.properties?.alt, node.properties?.src)` → `imageLabel(node.properties?.alt, undefined)` | KILLED: DS18 blank image alt uses a readable URL in standalone and nested links (14.071458ms) |

최종 좁은 DOM·CSS 회귀 19/19, 타입·소스 린트·리뷰 형식 통과. 변경분 변이는 종료 0이며 실행 Rust 변이는 없다.
