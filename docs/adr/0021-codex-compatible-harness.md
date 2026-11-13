# ADR 0021: Codex 호환 하네스와 수정 1차

- 상태: 제안
- 날짜: 2026-11-13
- 관련 작업: H-41
- 수정 1차 기준: 5ef5dfc
- 이 결정의 Codex·역할 절차가 ADR 0017~0020의 Claude 전용 절차보다 앞선다.

## 설치 버전과 실제 계약

Codex CLI 0.168.0을 썼다. 홈 설정과 실제 자격 증명은 읽거나 바꾸지 않았다. 설치 바이너리의 --codex-run-as-apply-patch로 실제 파서를 모델 없이 실행했다.

| 입력 | 실제 파서 |
|---|---|
| 들여쓴 표시 줄·끝 공백·CRLF·표시 줄 바깥 공백 | 모두 적용 |
| 앞쪽 임의 텍스트·여러 패치 블록·모르는 표시 줄 | 모두 거부 |
| 인용한 EOF heredoc 래핑 | 적용 |

가드는 표시 줄을 trim하고 CRLF를 처리한다. 경로의 .·.. 조각은 실제 적용과 같이 정규화한 뒤 세션 범위와 공유 편집 규칙을 검사한다. 경로에 공백·제어 문자가 있으면 적용기의 정규화에 맡기지 않고 거부한다. 모르는 표시와 여러 블록도 거부한다.

훅의 process.cwd는 세션 루트였다. 도구 workdir=child일 때 입력 cwd만 child로 달라졌다. 실제 관찰한 도구 이름은 Bash·apply_patch다. 역할과 상대 편집 기준은 프로세스 폴더에서 찾은 등록 worktree다. Git 환경 변수로 이를 돌릴 수 없다.

### PreToolUse 종료 형태

첫 계약 실험은 없는 파일의 Update여서 부작용 판정이 미확정이었다. 둘째는 기존 base 내용을 probe로 바꿔 파일 내용으로 확인했다.

| 출력·종료 | 실제 파일 | 판정 |
|---|---|---|
| 출력 없음, 0 | probe | 허용 |
| permissionDecision=deny JSON, 0 | base | 거부 |
| decision=block JSON, 2 | probe | 거부 실패 |
| stderr만, 2 | base | 이번 실측에서는 거부 |
| 처리하지 않은 예외 | probe | 거부 실패 |
| 잘못된 JSON, 0 | probe | 거부 실패 |

예외는 표준 permissionDecision=deny JSON·종료 0으로 처리하고 stderr에 사유를 쓴다. 예전의 “예외는 exit 2와 거부 JSON” 설명은 틀렸다. 파일 없음·시작 실패·시간 초과는 스크립트가 처리할 수 없어 여전히 차단을 보장하지 못한다.

## 수정과 근거

| 지적 | 결정 | 테스트 |
|---|---|---|
| 1·2 | 표시·블록·경로를 엄격히 검사 | R1 indented CRLF markers, R2 whitespace and controls |
| 3 | 예외도 표준 deny·종료 0·stderr | R3 dispatch exceptions |
| 4 | 프로세스 cwd·git-dir/common-dir·등록 경로 확인, 작업 사본 밖 거부 | R4 session role, R4 git environment |
| 5 | 저장소는 휴대 가능한 정책만, 컴퓨터별 값은 setup 출력 | R5 repository config |
| 6 | 주 작업 트리의 코드·규칙을 실행·조회. 공유 protectedDenies·ADR 목록으로 worker의 하네스·린트·테스트·도구 설정을 기본 읽기 전용으로 생성 | R6 hook commands, R6 worktree hook tampering |
| 7 | Bash의 정규화된 실행 파일 이름과 단독 인용 heredoc을 같은 판정·린트에 전달. 인용 연결·이스케이프·대문자 변형도 판정. 동적 입력과 폴더 변경 래퍼·복합 명령 거부 | R7 Bash heredoc |
| 8 | T3 키·값 허용 목록과 소유 스레드 기록 | R8 T3 launch |
| 9 | docs/agents를 보호 경로로 | R9 implementer instructions |
| 10 | 소유 스레드 수정 기준은 등록 worktree 이력. 진행 문서는 메인만 편집 | R10 only owned threads |
| 11 | Claude 셸 금지·종료 훅 한계·통제·오염 방지 설명 복구 | 현재 Claude 훅과 문서 대조 |
| 12 | 감시 래퍼 제거. 운영 훅 감사 기록과 실제 파일·실행 표식 비교 | regression requires every actual attempt |
| 13 | 모르는 도구 기본 거부. 실행 도구는 Bash 판정 | R13 unknown executable tools |
| 14 | 폴더에서 해시 대상 자동 계산, 고정 대상 폴더도 별도 단언 | R14 new guard files, R14 hash folder coverage |
| 15 | 하위 AGENTS.md·AGENTS.override.md 위치 검사 | R15 nested guidance |
| 16 | 서브에이전트 필드 각각 검사, 이동·삭제·상대·공백 예제 | R16 either subagent field, R16 moving deleting |

## 저장소와 컴퓨터별 부분

| 위치 | 내용 |
|---|---|
| 추적하는 .codex/config.toml | 홈 축약 경로·네트워크 도메인·공유 허용 목록에서 계산한 보호 경로 읽기 전용. 사용자 이름·Node 설치·worktree Git 경로 없음 |
| 추적하는 hooks.json | 환경을 비운 git 질의로 공통 폴더를 찾아 원본 dispatch 실행 |
| just codex-setup 출력 | PATH·pnpm_config_store_dir와 자기 Git 관리 폴더·objects·브랜치 ref·lock·log 쓰기, 검사 임시 폴더의 Unix 소켓 범위를 가진 stock-copilot-session |
| just codex-setup --harness 출력 | lock off에서만 사용자 승인 하네스 작업용 stock-copilot-harness 생성 |
| 홈 설정 | 사용자가 출력 내용을 검토해 반영한다. 에이전트는 자동으로 쓰지 않는다 |

컴퓨터별 프로필은 stock-copilot을 상속한다. 사용자가 default_permissions=stock-copilot-session을 명시적으로 선택한다. sandbox 재현은 -P stock-copilot-session이다. 승인한 하네스 작업에서만 --harness로 출력하고 그 프로필을 선택한다. 생성 결과를 커밋하지 않는다.

[공식 권한 문서](https://learn.chatgpt.com/docs/permissions#local-and-private-networks)와 실측을 대조했다. 기본 프로필과 localhost 도메인만 추가한 프로필은 TCP bind를 거부했다. 공유 정책의 allowLocalBinding=true를 Codex allow_local_binding에 그대로 반영한다. Unix 소켓은 전부 열지 않고 컴퓨터별 출력에 공유 checkPlaces가 쓰는 실제 임시 폴더만 허용한다. TCP·Unix 자체 서버 연결 성공과 1.1.1.1 직접 연결 EPERM을 확인했다. 이 예외는 검사에 필요한 로컬 통신 범위이며 외부 도메인 허용 목록은 유지한다.

임시 worktree에서 좁은 Git 권한으로 add·commit·merge master가 모두 종료 0이었다. 전체 소스 사본에서는 Lefthook의 fresh-build·commit-guard·전체 check를 모두 켠 commit도 종료 0이고 HEAD가 바뀌었다. hooks·config 쓰기는 모두 거부됐다. 자기 Git 관리 폴더의 commondir·gitdir·config.worktree도 읽기 전용으로 별도 제한했다. 제한 전에는 세 파일 쓰기가 성공했고 제한 후에는 모두 EPERM이었다. add·commit·merge의 성공과 두 부모 병합은 그대로 확인했다. 자동 유지보수는 packed-refs.lock 쓰기 경고를 냈지만 커밋·병합은 성공했다. 다른 브랜치를 바꿀 수 있는 packed-refs 쓰기는 열지 않는다.

## 역할과 T3 흐름

메인은 이 프로젝트에서 시작하고 projectId·environmentId를 넘기지 않는다. 초기 구현은 t3_thread_launch의 현재 프로젝트 상속·provider=codex·model=gpt-6.1-sol·worktree·브리프 기준으로 시작한다. runtimeMode는 생략 또는 approval-required다. options는 생략 또는 effort=high만 허용한다.

PostToolUse가 시작 결과의 threadId와 브리프 브랜치를 메인 세션 소유 기록에 넣는다. 기록과 등록 worktree가 있는 스레드만 read·wait·send로 접근한다. 수정은 같은 스레드의 send(auto 또는 queue)로 계속한다. 기준은 master 끝의 자손이면서 그 worktree HEAD의 조상이어야 한다. 메인이 앞 구현의 검사·리뷰 결과를 확인한다. Stop 전체 검사 성공과 깨끗한 HEAD는 기존 passedTargets 형식으로 기록한다. 수정 브리프는 소유 스레드 이력을 확인하므로 앞 회차의 Claude 서브에이전트 기록 없이도 이어 간다. 적대적 리뷰는 매번 새 Claude Opus 5.5다. 이전에 시작한 스레드를 자동으로 채택하지 않는다.

## 병합 뒤 메인의 1분 적용 확인

1. 원본에 병합한 뒤 새 T3 Codex worktree 세션을 연다. 프로젝트 신뢰·훅 정의 신뢰·선택된 프로필을 확인한다.
2. Bash의 workdir를 새 세션 worktree로 명시한 pwd는 그 worktree를 출력해야 한다. /var와 /private/var 같은 실제 경로 별칭은 같은 폴더로 판정한다.
3. git -c core.hooksPath=/tmp status를 한 번 시도한다. 조회 명령이지만 금지 설정 사유로 거부돼야 한다. 실제 status 출력이면 훅 미적용이다.
4. 도구 workdir를 원본으로 한 pwd는 세션 범위 사유로 거부돼야 한다.
5. 자기 worktree의 harness/.h41-profile-probe 생성은 정상 프로필에서 실패하고 파일이 없어야 한다. 생기면 프로필 미적용이며 메인이 이 새 표식만 지운다.
6. 첫 1분은 적용 확인이다. Stop 전체 검사 성공은 별도로 기다린다. full-access가 프로필을 덮으면 적용됐다고 판정하지 않는다.

## 알려진 빈틈

- 시간 초과·명령 파일 없음·프로세스 시작 실패는 스크립트가 처리할 수 없고 차단을 보장하지 못한다.
- 프로젝트 신뢰와 훅 정의 신뢰는 별개이며 프로젝트 파일만으로 사용자 신뢰를 강제할 수 없다.
- T3 full-access가 프로필을 덮을 수 있다. 현재 세션은 새 설정으로 다시 시작하지 않았으므로 적용됐다고 판정하지 않는다.
- ConfigChange 이벤트가 없어 설정의 세션 반영 거부를 Claude와 같은 방식으로 구현하지 못한다.
- 일부 hosted·특수 도구는 훅 경로 밖이다. 훅을 거치지 않는 호출은 기본 거부 판정도 받지 않는다.
- 문자열 검사로 프로그램 내부의 임의 쓰기를 전부 해석하지 못한다. OS 권한과 보호 파일 서명 검사를 함께 쓴다.
- write_stdin은 이미 허용된 실행의 입력 채널이며 새 PreToolUse가 없다.
- H-20 전 같은 사용자 권한의 샌드박스 밖 프로세스는 상태 기록을 위조할 수 있다.
- 모델이 시도하지 않으면 회귀는 미확정이다. 감사 기록 입력 해시와 실제 결과를 함께 확인한다.
- 운영 회귀는 운영 hooks.json·프로필을 쓰지만 축소 사본이다. 전체 소스 사본의 운영 Stop 성공도 별도로 확인했으나 현재 T3 세션에 재시작 없이 소급 적용되지는 않는다.
- Git 자동 유지보수의 packed-refs.lock 경고는 남긴다. 전역 ref 쓰기는 열지 않는다.
- 컴퓨터별 설정은 출력만 한다. 사용자 적용과 새 worktree별 재출력이 필요하다.
- 읽기 도구 허용 목록은 API에서 확인한 view_image·MCP 자원 조회다. CLI에서 실제 이벤트로 관찰한 이름은 Bash·apply_patch다. 새 도구는 확인 뒤 추가한다.

## 결과

수정 1차 헤드리스는 계약 2회·운영 회귀 2회·전체 소스 Stop 3회로 7회다. 운영 회귀는 마지막 실행도 실제 상태 기준 10/10 통과했다. 첫 전체 Stop에서 테스트의 중첩 샌드박스와 T3 결과 해석 결함을 발견해 수정했다. 둘째 전체 Stop은 status=passed·깨끗한 HEAD를 기록했다. 셋째는 실제 경로 별칭 보정 뒤 pwd 성공·Stop status=passed·깨끗한 HEAD를 함께 확인했다. 패치·Git 권한·HOME 변경 검사는 모델 없이 했다. Codex의 실제 Seatbelt 셸은 CODEX_SANDBOX=seatbelt를 준다. justfile은 이를 기존 SANDBOX_RUNTIME=1 계약에 연결해 중첩 불가 환경을 인식한다. 샌드박스 밖에서는 기존 표식의 값이 유지된다. 실제 OS 권한을 풀거나 테스트 단언을 바꾸지 않는다. pnpm 12 실측에서 npm_config_store_dir·PNPM_STORE_DIR는 적용되지 않고 pnpm_config_store_dir만 적용됐다. 자동 저장소 선택은 프로젝트와 검사 임시 사본에서 서로 다른 폴더를 골라 offline tarball 실패를 냈다. 컴퓨터별 출력에서 이 환경 변수로 허용된 홈 캐시를 고정하고 config get store-dir와 실제 오프라인 도구 복구 테스트로 확인한다. 홈 설정·키·계좌 데이터를 다루지 않았다. 검사 끄기는 추가하지 않았다.

known-gaps: 63건 (eslint 7건, hook 6건, adr 50건(문서만(예제 없음)))
