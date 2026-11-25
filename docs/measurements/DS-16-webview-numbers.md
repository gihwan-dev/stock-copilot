# DS-16 실측 기록: WKWebView의 숫자 형식과 한글 조합 입력 (S-05)

- 작업: [tasks.md](../tasks.md) DS-16, 실측 [open-issues.md](../open-issues.md) S-05. 관련 ADR 0079
- 기록일: 2026-11-25 (fin-measure 묶음), macOS 27.2
- 도구: [ds16-webview.swift](ds16-webview.swift)

## 방법

Playwright WebKit은 이 PC에 설치돼 있지 않다(Chromium만 있음). 대신 앱과 같은 엔진인 시스템 `WKWebView`를 직접 띄우는 Swift 도구를 썼다.

1. `apps/desktop/src/shared/lib/format.ts`의 실제 `formatNumber`·`formatPriceChange`를 esbuild로 묶어 Node에서 돌려 표시 문자열을 뽑았다.
2. 그 문자열을 앱과 같은 글꼴 묶음(`tokens.css`의 `--font-sans`)과 `font-variant-numeric: tabular-nums`(`Numbers.tsx`의 `tabular-nums`)로 WKWebView에 그리고, 글자 폭을 재고 화면을 떴다.
3. WKWebView의 `NSTextInputClient`(`setMarkedText`·`insertText`)로 한글 조합을 흉내 내 `textarea`의 조합 이벤트 순서를 기록했다.

```sh
swiftc -O docs/measurements/ds16-webview.swift -o <임시>/ds16-webview
<임시>/ds16-webview <cases.json> <snapshot.png>
FONT_STACK='system-ui, "Apple SD Gothic Neo", sans-serif' <임시>/ds16-webview <cases.json> <snapshot.png>
```

## 숫자 형식 결과

| 입력 | 표시 문자열 | 판정 |
|---|---|---|
| 1234567원 | `1,234,567원` | 맞음 |
| −1234567원, 하락 | `−1,234,567원` | 맞음 (U+2212) |
| 50000원, 상승 | `+50,000원` | 맞음 |
| 123456789012345원, 축약 | `123조 4,567억 8,901만 2,345원` | 맞음 |
| 12345678원, 축약 | `1,234만 5,678원` | 맞음 |
| 100000000원, 축약 | `1억원` | 맞음 |
| −2500000000원, 축약, 하락 | `−25억원` | 맞음 |
| −1234.56 USD | `−$1,234.56` | 맞음 |
| 1200주 | `1,200주` | 맞음 |
| −3.25%, 하락 | `−3.25%` | 맞음 |
| 가격 변화 | `−1,500원 (−2.10%)` | 맞음 |

- 쉼표, 만·억·조, 단위: 문서 예제와 같게 그려졌다.
- 빼기: 표시 문자열 전체에 U+2212가 6개, 하이픈(U+002D)은 0개. U+2212의 폭은 8.862px로 빈 글리프(12.11px)와 다르므로 실제 글리프가 그려졌다. 다만 U+2212가 든 줄의 높이가 18.08px로 다른 줄(17px)보다 크다. Apple SD Gothic Neo에 U+2212가 없어 다른 글꼴로 대체된 것으로 보인다. 눈에 띄는 어긋남은 없었다.

## 고정폭 숫자: **문서와 다름**

| 글꼴 묶음 | 실제로 쓰인 글꼴 | 숫자 0~9 폭(14px) | 고정폭 |
|---|---|---|---|
| 앱 그대로: `"IBM Plex Sans KR", "Apple SD Gothic Neo", …` | Apple SD Gothic Neo (IBM Plex Sans KR은 설치돼 있지 않고 앱도 `@font-face`로 싣지 않음) | 5.14 ~ 8.86px로 제각각 | **아님**. `1,111,111원` 52.63px, `8,888,888원` 74.52px |
| `system-ui, "Apple SD Gothic Neo", …` | SF Pro | 모두 8.862px | 맞음. 같은 자릿수면 같은 폭 |

`tabular-nums`는 글꼴에 `tnum` 기능이 있어야 동작한다. 앱의 실제 대체 글꼴인 Apple SD Gothic Neo에는 없어서, 표의 금액 열이 자릿수가 같아도 폭이 달라 세로로 줄이 맞지 않는다. 문서(ADR 0079, R-14)의 "고정폭 숫자"와 다르다.

**DS-05 수정 작업이 필요하다** (tasks.md는 메인이 고친다). 고칠 방향 후보:

1. IBM Plex Sans KR을 앱에 글꼴 파일로 싣는다(`@font-face`, 라이선스 OFL). 디자인 산출물과 같아진다.
2. 숫자 표시에만 `system-ui`(SF Pro)를 앞에 둔다. 한글 단위(원·주·만·억·조)는 다음 글꼴로 넘어간다.

## 한글 조합 입력

`ㅎ → 하 → 한` 조합 뒤 확정:

`compositionstart` → (`compositionupdate`, `beforeinput`, `input`)×3 → 확정 `beforeinput`·`input` → `compositionend("한")`. 조합 중 `input`의 `isComposing`은 `true`였다. `ChatInput`의 `compositionstart`/`compositionend` 상태 전환이 의도대로 일어난다.

조합 중(`글`이 표시된 상태) Enter 키:

`keydown(Enter, keyCode 13, isComposing: true)` → `beforeinput` → `compositionend("")` → `input`(줄바꿈) → `keyup`.

- `keydown`이 `compositionend`보다 먼저 오고 `isComposing`이 참이다. `ChatInput`은 이때 상태가 `composing`이라 보내지 않는다.
- 이 순서는 실제 입력기 없이 조합 글자만 밀어 넣은 결과다. 실제 macOS 한글 입력기에서는 Enter를 입력기가 먼저 받아 조합을 끝내므로 순서(특히 Safari 계열의 `compositionend` → `keydown(229)`)가 다를 수 있다. 위 결과는 조합 이벤트가 WKWebView에서 발생한다는 것까지만 증명한다.

## 남은 사람 몫

- 실제 설치 앱 창에서 한글 입력기로 채팅 입력에 "한글" 입력 후 Enter: 조합 중 Enter로 보내지지 않고, 조합이 끝난 뒤 Enter로 보내지는지. 조합 중 마지막 글자가 사라지지 않는지.
- DS-05 수정(위 고정폭) 뒤 실제 창에서 금액 열이 세로로 맞는지 눈으로 확인.
