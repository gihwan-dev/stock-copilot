# ADR 0179: 실측 뒤에도 수집 실행에 WebFetch를 허용하지 않는다 (AI-15, 실측 S-07·S-04)

- 상태: 확정 (2026-11-25, fin-aiiso 묶음). [ADR 0178](0178-collection-web-reading.md)의 "다시 볼 조건"을 실측 결과로 다시 본 결정이며 0178의 결정을 유지한다.

## 배경

ADR 0178은 수집 실행을 `--restricted --tools WebSearch`로 두고, 사용자 실측에서 `restricted-webfetch`가 WebFetch를 남기면서 격리를 통과하면 다시 정하기로 했다. 시험 조합은 [claude-isolation-check.sh](../measurements/claude-isolation-check.sh)에 있다.

## 실측 결과

메인 세션이 claude 2.1.332, 모델 haiku로 세 조합을 돌렸다(fin-aiiso 묶음). AI-12 실측도 같은 실행이다. 자세한 값은 [AI-12 실측 기록](../measurements/AI-12-claude-isolation.md).

| 날짜 | 조합 | init tools | skills·slash_commands | 훅 | 전역 지침·메모리 | WebFetch 동작 |
|---|---|---|---|---|---|---|
| 2026-11-25 | `app-collection` | `[WebSearch]` | `[]`·`[]` | 0건, 표식 없음 | 주입 없음 | 없음 |
| 2026-11-25 | `restricted-webfetch` | WebSearch, WebFetch | `[]`·`[]` | 0건, 표식 없음 | 주입 없음 | 동작함 (example.com 제목을 읽음) |
| 2026-11-25 | `sources-none-webfetch` | WebSearch, WebFetch | - | 없음 | **사용자 메모리 지침 주입됨** | 동작함 |

세 조합 모두 init `plugins`는 내장 `cc-plugin-agents-md`, `cc-plugin-privacy`, `cc-plugin-plugin-authoring`였고 `apiKeySource=none`, `permissionMode=dontAsk`였다. 세 조합 모두 로그인 계정 식별자(이메일)가 시스템 알림 문맥으로 모델에 들어갔다.

문서의 예상과 달리 `--restricted`는 `--tools`로 넘긴 WebFetch를 없애지 않았다. 격리 조건만 보면 `restricted-webfetch`는 통과한다.

## 결정

수집 실행은 `--restricted --tools WebSearch`를 유지한다. `restricted-webfetch`가 격리를 지키며 웹을 읽는 것은 확인했지만, 수집 실행에 WebFetch를 허용하지 않는다. 웹 검색 결과(제목, 주소, 요약)만 쓰고, 기사 본문은 분석 실행의 `read_article`이 앱이 아는 주소만 가져온다(ADR 0178).

이유:

1. SEC-2는 수집 실행의 도구를 "내장 WebSearch 하나"로 정한다. 격리(훅·설정·스킬·메모리 차단)와 도구 범위는 다른 문제다. `restricted-webfetch`는 앞의 것만 통과한다.
2. WebFetch는 아무 주소에나 요청을 보낸다. 실측에서 계정 식별자가 문맥에 들어오는 것을 확인했으므로, 검색 결과 속 주입 문장이 `https://<바깥 서버>/?q=<계정 식별자·조사 문맥>` 같은 요청을 만들게 할 수 있다. 사용자가 채팅에 쓴 문장은 SEC-9 거부 규칙 없이 그대로 넘어오므로(design 4.4) 이 문맥도 같은 길로 샐 수 있다. WebSearch만 있으면 요청은 구독 제공자의 검색으로만 나간다.
3. 앱이 가져오는 `read_article`은 언론사 RSS 도메인만 읽는다(design 9장 위험 표). WebFetch에는 앱 코드가 강제하는 도메인 제한이 없다.
4. WebFetch는 원문 대신 별도 모델의 요약을 넘기므로(R-02) 본문 근거로서의 가치가 낮다.
5. 코드 변경이 없다. `command.rs`의 수집 실행 인자와 `command_env_ai03.rs`의 인자 고정 시험이 그대로 이 결정을 지킨다.

## 버린 대안

- `--restricted`를 빼고 WebFetch를 살리는 조합(`sources-none-webfetch`): 실측에서 사용자 메모리 지침이 주입됐다. SEC-2 위반이다.
- `--restricted`를 유지하고 WebFetch 허용(`restricted-webfetch`): 격리는 지키지만 위 1~3.
- `--allowedTools`에 `WebFetch(domain:…)`처럼 도메인을 묶어 허용: 제한을 CLI 권한 규칙에 맡기게 되고, `dontAsk`·`--restricted`와 함께 어떻게 동작하는지 실측이 없다. 필요해지면 앱이 도메인을 강제하는 가져오기 도구 쪽을 먼저 본다.
- 수집 실행에 앱의 가져오기 도구 주기: ADR 0178 이유 3(수집 실행에 MCP 경계를 하나 더 연다). 검색 요약만으로 수집 품질이 모자란다는 근거가 쌓이면 다시 본다.

## 다시 볼 조건

- 검색 요약만으로 수집 품질이 모자란다는 근거가 쌓이면, 앱이 도메인을 강제하는 가져오기 도구를 수집 실행에 줄지 다시 본다.
- Claude Code에 계정 문맥 주입을 끄는 옵션이 생기고 WebFetch의 주소를 앱이 정한 목록으로 묶을 수 있음이 실측되면 WebFetch를 다시 본다.
- Claude Code 버전을 올릴 때 [claude-isolation-check.sh](../measurements/claude-isolation-check.sh) `app-collection`을 다시 돌리고, 하나라도 어기면 격리 옵션부터 고친다.
