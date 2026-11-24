# 묶음 작업 규칙

경량 절차(ADR 0173)로 묶음을 구현하는 스레드가 따르는 규칙이다. 지시문에서 이 파일을 가리킨다.

## 규칙 (사용자 결정: 속도 우선, 2026-11-25 06:00까지 끝낸다)
- 환경: 모든 명령 앞에 `export PATH="$HOME/.nvm/versions/node/v24.22.0/bin:$HOME/.cargo/bin:$HOME/.local/harness-bin:$HOME/Library/pnpm:/opt/homebrew/bin:$PATH"; export TMPDIR=/tmp; export CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4`. 의존성은 `pnpm install --frozen-lockfile --ignore-scripts`. 푸시는 `/usr/bin/git push origin <네 브랜치>`만. 강제 푸시·rebase·amend 금지.
- `AGENTS.md`와 `docs/adr/0173-lightweight-process.md`를 읽는다. `docs/workflow.md`·`docs/agents/*`의 리뷰·기록·변이 절차는 쓰지 않는다. 작업별 ADR·리뷰 기록·손 변형·변이·커버리지 측정 없음.
- 행마다: `docs/tasks.md`의 그 행(완료 기준)과 가리키는 design·architecture 절만 읽고 구현 + 테스트(핵심 동작과 거부 경로) → 그 crate의 Clippy·nextest(프런트는 `tsc`·ESLint·vitest) → "<작업 ID>: <한 줄>" 제목으로 커밋.
- 합류(행마다): `/usr/bin/git fetch origin` → `git merge --no-ff --no-edit origin/master`(충돌은 양쪽 기능을 모두 살린다. 생성 파일은 master 쪽을 받고 `node harness/generated-files/check.mjs --write`와 `node harness/codex-hooks/config.mjs --write`, `node harness/sandbox/fragment.mjs --write`, `node harness/antigravity-hooks/config.mjs --write`, `node harness/claude-hooks/hashes.mjs --write`로 다시 만든다) → `cargo check --workspace --all-targets --locked`와 `pnpm -C apps/desktop exec tsc -p .` 통과 확인 → 푸시. **메인이 네 브랜치 끝으로 master를 그대로 빨리 감으므로 컴파일이 깨진 상태는 푸시하지 않는다.** `just check-lite` 전체(약 15분)는 3~4행마다 한 번과 묶음 끝에만 돌리고 실패를 고친다. `just check`·`check-fast`·`test-diff`는 돌리지 않는다.
- 멈추지 않고 묻지 않는다(질문 도구 금지): 문서가 정하지 않은 것은 가장 단순하고 안전한 쪽으로 정하고 `docs/adr/<묶음 이름>-decisions.md` 표에 (작업, 정한 것, 다른 선택지) 한 줄. 선행 작업이 master에 없으면 포트(trait)와 `testkit-ports`의 가짜로 댄다. "담당: 사용자"·실측 행은 건너뛴다.
- 보호 파일의 추가형 변경(허용 목록, 잠금 파일에 이미 있는 패키지의 전이 예외, 고정 목록의 새 항목, `arch-check`·`source-check`·`capability-check`가 요구하는 항목)과 그에 따른 고정 테스트 갱신은 그냥 한다. 세션 훅이 명령을 막으면 한 명령씩 다시 쓰고, 그래도 막히면 다른 길로 진행한다.
- 금지: 문서·ADR이 정하지 않은 새 외부 패키지(그 행은 건너뛰고 적는다), 문서가 정하지 않은 `unsafe`, 실제 토스 API·실제 주문 호출, 실제 `claude`·`codex` CLI 실행, 앱을 실제로 띄우는 명령, 실제 키·계좌 자료, 테스트 중 실제 외부 네트워크 호출, 사용자 홈의 설정·메모리·자격 증명 파일 읽기·수정, 저장소 파일에 사용자별 절대 경로, 소스 주석, `docs/progress.md`·`docs/tasks.md` 수정.
- 하위 에이전트는 Claude Opus(effort medium)만, 서로 겹치지 않는 파일을 병렬로 맡길 때 쓴다. 다른 제공자·모델은 쓰지 않는다.
- 턴을 끝내지 않는다. 백그라운드 명령은 알림을 기다리지 말고 직접 상태를 확인한다. 묶음 끝에 `cargo clean`.
- 보고(묶음 끝에 한 번): 첫 줄 "완료" 또는 "막힘:", 최종 SHA, 행별 표(작업, 구현/건너뜀과 이유, 커밋), 정한 것, 마지막 `check-lite` 결과.
