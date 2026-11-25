# H-67: 스테이징 상태에서 master 받기와 막힘 뒤 종료

- 상태: 확정 (2026-11-25, fin-b 묶음)
- 관련: H-61, ADR 0018, ADR 0173

## 결정

| 항목 | 정한 것 | 버린 대안 |
|---|---|---|
| master 받기 | `node harness/antigravity-hooks/staged-sync.mjs sync` 한 명령이 보관·병합·복원을 한 묶음으로 한다. 조건: 이름 있는 작업 브랜치(`master` 아님), 진행 중인 merge·cherry-pick·revert·rebase 없음, 스테이징하지 않은 추적 변경과 추적하지 않은 파일 없음, `origin/master` 있음. 스테이징 변경을 git 폴더의 패치로 남기고 HEAD로 맞춘 뒤 `git merge --no-ff --no-edit origin/master`, 이어서 `git apply --index --3way`로 복원한다. 병합 충돌이나 복원 충돌이면 원래 HEAD와 스테이징 상태로 되돌리고 실패한다. 병합 대상은 `origin/master`로 고정이다 | 승인 검증 기준을 커밋 시작 시점에 고정, 변경형 `git stash` 허용 |
| 막힘 뒤 종료 | `node harness/antigravity-hooks/staged-sync.mjs blocked <한 줄 이유>`가 작업 사본의 git 폴더에 `stock-copilot-blocked.json`(보고 첫 줄, HEAD, 상태 요약값)을 남긴다. 구현 담당의 종료 훅은 미커밋 변경이 있어도 이 기록의 HEAD와 상태 요약값(HEAD, `status -uall`, 작업·스테이징 diff, 추적하지 않은 파일 내용의 SHA-256)이 지금과 같을 때만 종료를 허용한다. Antigravity 종료 입력에 마지막 응답이 없어서 기록 파일로 판정한다 | 종료 훅 입력의 마지막 응답 첫 줄로 판정(필드가 없다) |
| 우회 예제 | 기록 뒤 파일을 더 고치거나, 추적하지 않은 파일을 더하거나, 커밋해 HEAD를 옮기면 종료가 다시 막힌다. 빈 이유·여러 줄 이유·첫 줄이 `막힘:`이 아닌 기록·깨진 기록은 거부한다. 셸로 git 폴더에 기록을 쓰는 것, 변경형 `git stash`, `git reset --merge`는 잠금과 무관하게 계속 거부된다(`harness/antigravity-hooks/tests/staged-sync.test.mjs`) | - |

## 남은 일

- `harness/claude-hooks/hashes.json`의 `harness/antigravity-hooks/*` 해시는 `hashes.mjs --write`가 사람 서명 커밋 전용이라(ADR 0018) 갱신하지 못했다. 사용자가 `node harness/claude-hooks/hashes.mjs --write`를 서명 커밋으로 넣어야 `config.test.mjs`의 해시 확인과 `check-fast-static`의 해시 검사가 통과한다.
- 이 경로는 Antigravity 구현 담당용이다. ADR 0173 뒤로 Antigravity는 쓰지 않으므로 실제 세션 실측은 하지 않았다.
