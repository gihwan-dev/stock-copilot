# ADR 0178: 수집 실행의 웹 읽기는 WebSearch 결과만 쓴다 (AI-15, 실측 S-07)

- 상태: 확정 (2026-11-25, fin-measure 묶음). 실측 S-07 결과가 아래 "다시 볼 조건"에 걸리면 고친다.

## 배경

수집 실행은 `--restricted`로 사용자 훅·플러그인·설정·CLAUDE.md를 막는다([R-02](../research/R-02-claude-code-runtime.md) 3장). `--restricted`는 WebFetch도 없앤다. 실측 S-07은 "격리를 유지하면서 웹 읽기를 살리는 옵션 조합"이 있는지 보고, 없으면 (1) 웹 검색 결과(제목, 주소, 요약)만 쓰기, (2) 앱이 대신 가져오는 도구를 수집 실행에 주기 중에서 고르라고 한다.

## 시험

시험할 조합은 [claude-isolation-check.sh](../measurements/claude-isolation-check.sh)에 셋으로 넣었다.

| 조합 | 옵션 | 기대 |
|---|---|---|
| `app-collection` | 현재 앱과 같음: `--restricted --tools WebSearch` | WebSearch만, 격리 통과 |
| `restricted-webfetch` | `--restricted --tools WebSearch,WebFetch` | 문서상 init `tools`에서 WebFetch가 빠짐 |
| `sources-none-webfetch` | `--restricted` 없이 `--setting-sources=` + `--settings {"disableAllHooks":true}` + `--tools WebSearch,WebFetch` | WebFetch는 살지만 플러그인·스킬·전역 지침·auto-memory 차단이 보장되지 않음 |

이번 묶음에서는 세션 훅이 에이전트의 `claude` 실행을 거부해(docs/harness.md 7.3, ADR 0017) 세 조합을 실제로 돌리지 못했다. 결과 표는 사용자가 채운다.

| 날짜 | 조합 | init tools | skills·slash_commands | 훅 | 전역 지침·메모리 | WebFetch 동작 |
|---|---|---|---|---|---|---|
| (사용자 실행 후) | | | | | | |

## 결정

수집 실행은 지금처럼 `--restricted --tools WebSearch`를 유지하고, 웹 검색 결과(제목, 주소, 요약)만 쓴다. 기사 본문이 필요하면 분석 실행의 `read_article` MCP 도구가 수집 목록에 있는 주소만 앱이 가져와 준다(R-02 6장).

이유:

1. 격리(SEC-2)는 `--restricted` 하나에 기대고 있다. 이를 빼는 조합은 `--setting-sources`와 `disableAllHooks`로 훅·설정을 막더라도 플러그인·스킬·auto-memory 차단을 따로 증명해야 하고, Claude Code 버전마다 다시 증명해야 한다.
2. WebFetch는 원문 대신 별도 모델의 요약을 넘기므로(R-02) 본문 근거로서의 가치가 낮고, 임의 주소를 읽어 프롬프트 주입 면을 넓힌다.
3. 앱이 대신 가져오는 도구를 수집 실행에 주면 수집 실행에도 MCP 연결이 생긴다. 지금 코드는 수집 실행에 MCP를 붙이면 거부한다(`CommandBuildError::McpAttachedToCollection`). 경계를 하나 더 여는 일이라 실측 근거 없이 하지 않는다.
4. 코드 변경이 없다.

## 버린 대안

- `--restricted`를 빼고 WebFetch를 살리는 조합: 위 1, 2.
- 수집 실행에 앱의 가져오기 도구 주기: 위 3. 검색 요약만으로 수집 품질이 모자란다는 근거가 쌓이면 다시 본다.

## 다시 볼 조건

- 사용자 실측에서 `restricted-webfetch`가 WebFetch를 남기고 격리 조건을 모두 통과하면, 그 조합을 쓸지 다시 정한다.
- `app-collection`이 격리 조건 하나라도 어기면 AI-12에서 격리 옵션부터 고친다.
