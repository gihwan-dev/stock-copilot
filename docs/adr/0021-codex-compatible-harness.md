# ADR 0021: Codex 호환 하네스

- 상태: 제안
- 날짜: 2026-11-13
- 관련 작업: H-41
- 수정 1차 기준: 5ef5dfc
- 이 결정의 Codex·역할 절차가 ADR 0017~0020의 Claude 전용 절차보다 앞선다.

## 설치 버전과 실제 계약

Codex CLI 0.168.0을 썼다. 수정 1차의 홈 신뢰 항목 잔류는 수정 3차에서 정정한다. 실제 자격 증명은 다루지 않는다. 설치 바이너리의 --codex-run-as-apply-patch로 실제 파서를 모델 없이 실행했다.

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

[공식 권한 문서](https://learn.chatgpt.com/docs/permissions#local-and-private-networks)와 실측을 대조했다. 기본 프로필과 localhost 도메인만 추가한 프로필은 TCP bind를 거부했다. 공유 정책의 allowLocalBinding=true를 Codex allow_local_binding에 그대로 반영한다. Unix 소켓은 전부 열지 않고 컴퓨터별 출력에 세션 worktree 전용 검사 임시 폴더만 허용한다. 셸과 checkPlaces의 TMPDIR도 같은 폴더로 고정한다. TCP·Unix 자체 서버 연결 성공과 1.1.1.1 직접 연결 EPERM을 확인했다. 이 예외는 검사에 필요한 로컬 통신 범위이며 외부 도메인 허용 목록은 유지한다.

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

- 래퍼 자체의 시작 실패·강제 종료·시간 초과는 deny 출력을 보장하지 못한다. Git 질의 실패·Node 실패·모듈 import 실패는 래퍼가 이벤트별 거부 JSON과 종료 0으로 닫는다.
- 프로젝트 신뢰와 훅 정의 신뢰는 별개이며 프로젝트 파일만으로 사용자 신뢰를 강제할 수 없다.
- T3 full-access가 프로필을 덮을 수 있다. 2026-11-13 메인의 새 T3 세션 검증은 훅·프로필 미적용이었다. 프로젝트 신뢰와 실제 적용 확인을 마치기 전에는 강제로 보지 않는다.
- ConfigChange 이벤트가 없어 설정의 세션 반영 거부를 Claude와 같은 방식으로 구현하지 못한다.
- 일부 hosted·특수 도구는 훅 경로 밖이다. 훅을 거치지 않는 호출은 기본 거부 판정도 받지 않는다.
- 문자열 검사로 프로그램 내부의 임의 쓰기를 전부 해석하지 못한다. OS 권한과 보호 파일 서명 검사를 함께 쓴다.
- write_stdin은 이미 허용된 실행의 입력 채널이며 새 PreToolUse가 없다.
- H-20 전 같은 사용자 권한의 샌드박스 밖 프로세스는 상태 기록을 위조할 수 있다.
- 모델이 시도하지 않으면 회귀는 미확정이다. 감사 기록 입력 해시와 실제 결과를 함께 확인한다.
- 운영 회귀는 운영 hooks.json·프로필을 쓰지만 축소 사본이다. 전체 소스 사본의 운영 Stop 성공도 별도로 확인했으나 현재 T3 세션에 재시작 없이 소급 적용되지는 않는다.
- Git 자동 유지보수의 packed-refs.lock 경고는 남긴다. 전역 ref 쓰기는 열지 않는다.
- 컴퓨터별 설정은 출력만 한다. 사용자 적용과 새 worktree별 재출력이 필요하다. 하네스 프로필은 자동 회수되지 않는다. 종료 후 사용자가 지우고 잠금 전에 명시적 사용자 설정 검사를 한다.
- 읽기 도구 허용 목록은 API에서 확인한 view_image·MCP 자원 조회다. CLI에서 실제 이벤트로 관찰한 이름은 Bash·apply_patch다. 새 도구는 확인 뒤 추가한다.

## 결과

수정 1차 헤드리스는 계약 2회·운영 회귀 2회·전체 소스 Stop 3회로 7회다. 운영 회귀는 마지막 실행도 실제 상태 기준 10/10 통과했다. 첫 전체 Stop에서 테스트의 중첩 샌드박스와 T3 결과 해석 결함을 발견해 수정했다. 둘째 전체 Stop은 status=passed·깨끗한 HEAD를 기록했다. 셋째는 실제 경로 별칭 보정 뒤 pwd 성공·Stop status=passed·깨끗한 HEAD를 함께 확인했다. 패치·Git 권한·HOME 변경 검사는 모델 없이 했다. Codex의 실제 Seatbelt 셸은 CODEX_SANDBOX=seatbelt를 준다. justfile은 이를 기존 SANDBOX_RUNTIME=1 계약에 연결해 중첩 불가 환경을 인식한다. 샌드박스 밖에서는 기존 표식의 값이 유지된다. 실제 OS 권한을 풀거나 테스트 단언을 바꾸지 않는다. pnpm 12 실측에서 npm_config_store_dir·PNPM_STORE_DIR는 적용되지 않고 pnpm_config_store_dir만 적용됐다. 자동 저장소 선택은 프로젝트와 검사 임시 사본에서 서로 다른 폴더를 골라 offline tarball 실패를 냈다. 컴퓨터별 출력에서 이 환경 변수로 허용된 홈 캐시를 고정하고 config get store-dir와 실제 오프라인 도구 복구 테스트로 확인한다. 키·계좌 데이터는 다루지 않았다. 홈 설정을 바꾸지 않았다는 당시 보고는 수정 3차에서 정정한다. 검사 끄기는 추가하지 않았다.

known-gaps: 63건 (eslint 7건, hook 6건, adr 50건(문서만(예제 없음)))


## 수정 2차 결정과 실측

대상은 865b791이다. Codex CLI는 0.168.0이다. 헤드리스 계약 2회와 운영 회귀 1회를 썼다. 패치 적용기·app-server 조회·Git 권한은 모델 없이 확인한다.

| 지적 | 조치 | 재현 테스트 |
|---|---|---|
| 1 | ASCII 공백·탭만 trim한다. 모든 줄을 엄격한 머리줄·표시·hunk로 분류하고 나머지는 거부한다 | R2 patch classification rejects every Rust whitespace header disguise and unknown body line |
| 2 | 래퍼와 dispatcher 예외를 이벤트별 거부 JSON·종료 0으로 닫는다. gitdir 전체 쓰기를 없앤다 | R2 wrapper emits event appropriate closed failure for broken HEAD and module import; R2 session Git writes exclude HEAD and the administrative directory grant |
| 3 | 공유 코어에서 codex·patch·xargs, git apply·am, find 쓰기·삭제·실행을 거부한다 | R2 shared shell core denies opaque writers and find side effects for both adapters; 셸 예제의 원소 빼기 검사 |
| 4 | T3 전체 matcher에서 기본 거부한다. send는 auto·생략만 쓴다 | R2 all T3 mutations are gated and queued sends are refused |
| 5 | 로컬은 SHA·false, 원격은 origin/master·true와 같은 원격 SHA의 브리프를 쓴다 | R2 local unpublished and origin launch bases have distinct enforced modes |
| 6 | 모든 파일·소켓 정책에 적용·상속·더 좁은 제한을 명시한다. 미분류 항목은 실패한다 | R2 every filesystem and socket policy entry has explicit coverage including non home entries |
| 7 | git ls-files만 해시한다. 새로 스테이징한 가드는 포함하고 미추적 잡음은 제외한다 | R2 hashes ignore untracked noise but include staged guard files |

Rust 공백은 0..=0x10ffff의 유효 char를 char::is_whitespace로 계산한다. 수동 목록은 없다. 같은 집합으로 직접 패치와 Bash heredoc을 검사한다. 정상 한글 hunk와 +로 시작하는 별표 본문은 허용한다. 실제 적용기에서 NEL을 붙인 첫 보호 대상 머리줄과 정상 둘째 머리줄을 함께 적용하면 두 파일이 바뀌었다. 865b791 어댑터는 이를 허용했고 수정 어댑터는 거부했다.

### 실패 출력의 실제 계약

| 실패 위치 | PreToolUse | Stop |
|---|---|---|
| Git rev-parse 실패 | permissionDecision=deny, 종료 0 | decision=block, 종료 0 |
| Node 실행·모듈 import 실패 | 같은 deny | 같은 block |
| dispatcher 내부 예외 | 같은 deny | 같은 block |
| 래퍼 자체 시작·시간 초과 | 출력 보장 못 함 | 출력 보장 못 함 |

임시 원본에서 dispatch 파일을 찾지 못하게 했다. 운영 Stop 래퍼의 block·종료 0은 첫 완료를 막았고 모델은 시작 실패 피드백 뒤 막힘을 보고했다. 실험용 훅만 둘째 Stop을 허용해 끝냈다. 실제 래퍼에는 이 예외가 없다.

### A. 시스템 임시 폴더 밖의 Git 권한

독립 임시 저장소와 두 worktree를 만들었다. worktree는 ~/.t3/worktrees/stock-copilot/ 아래이며 /private/tmp 밖이다. 공통 Git 폴더도 시스템 임시 폴더 밖이다.

| 시도 | 실제 결과 |
|---|---|
| add·서명 없는 commit·merge master | 모두 종료 0, 두 부모 병합 |
| refs/heads/master 이동 | 종료 128, 원래 SHA 유지 |
| 다른 브랜치 이동·새 브랜치·태그 생성 | 모두 종료 128 |
| packed-refs 쓰기 | 종료 1 |
| 다른 worktree 관리 폴더·소스 쓰기 | 모두 종료 1 |
| 주 작업 트리 파일 쓰기 | 종료 1, 원래 내용 유지 |
| 자기 HEAD·공통 config·hooks 쓰기 | 모두 종료 1 |
| 허용된 lock 파일로 HEAD·master를 원자 교체 | 모두 종료 1 |

자기 관리 폴더는 index·lock, HEAD.lock, ORIG_HEAD·lock, MERGE_HEAD·MSG·MODE·RR·AUTOSTASH, AUTO_MERGE·lock, COMMIT_EDITMSG, SQUASH_MSG, logs/HEAD·lock만 쓴다. HEAD.lock이 없으면 commit이, AUTO_MERGE.lock이 없으면 merge가 실패했다. 둘을 추가한 뒤 성공했다. HEAD 본문과 관리 폴더 전체 쓰기는 열지 않는다. 공통 Git 폴더는 objects와 자기 브랜치 ref·lock·log만 쓴다. 실험 뒤 git worktree remove --force로 두 임시 worktree를 지웠다.

### B. 하네스 프로필의 수명

잠금 off는 사용자 승인 자체가 아니다. --harness는 승인한 하네스 작업에서만 쓴다. 생성기는 사용자 설정을 변경하지 않으며 이미 적용한 권한을 자동 회수할 수도 없다.

| 시점 | 사용자가 할 일 |
|---|---|
| 시작 | 승인한 worktree에서 just codex-setup --harness 출력을 검토해 적용 |
| 종료 | 사용자 설정의 stock-copilot-harness 정의·선택 값을 삭제하고 일반 프로필로 새 세션 시작 |
| 잠금 on 전 | node harness/sandbox/prelock.mjs <검토 SHA> --codex-user-config 직접 실행 |
| 별도 설정 계층 | --codex-user-config <파일>로 각 유효 파일을 따로 확인 |

prelock은 명시적 --codex-user-config가 있을 때만 사용자 설정을 읽는다. 생성 프로필 이름이 남으면 실패한다. 모듈 import·기본 prelock·just check는 실제 사용자 설정을 읽지 않는다. 테스트는 임시 가짜 설정만 읽는다. 이미 실행 중인 세션의 권한을 회수하는 장치는 아니다.

### C. 설정과 훅 정의의 출처

설치된 app-server의 config/read·hooks/list를 임시 CODEX_HOME에서 실행했다. 실제 홈 설정·자격 증명을 복사하거나 읽지 않았다. 네이티브 CLI 두 번은 임시로 검토한 훅만 신뢰 우회 옵션으로 활성화했다. 사용자 신뢰 저장소는 변경하지 않았다.

| 관찰 | 결과 |
|---|---|
| config/read의 프로젝트 계층 | 세션 worktree/.codex, 원본 high 대신 worktree low |
| 실제 Bash의 설정 환경 변수 | 원본 PRIMARY 대신 WORKER 출력 |
| hooks/list의 sourcePath·command·hash | 주 작업 트리/.codex/hooks.json의 PRIMARY |
| worktree 훅을 WORKER_FIRST에서 WORKER_CHANGED로 수정 | 원본 훅 sourcePath·hash 유지 |
| 실제 PreToolUse·Stop 기록 | 수정 전후 모두 원본 훅 실행, cwd는 worker |
| 훅 신뢰 | 임시 저장소에서는 untrusted. 프로젝트 신뢰와 별개 |

0.168.0은 설정과 훅 정의의 기준 폴더가 다르다. 원본 훅 정의를 신뢰하고 매 worktree 설정·프로필을 확인한다. 다른 버전에서는 다시 실측한다. [공식 설정 문서](https://learn.chatgpt.com/docs/config-file/config-advanced#hooks)의 일반 설명만으로 worktree 출처를 추정하지 않는다.

### 정책 대응과 해시 생성

policy-coverage.json은 공유 정책의 모든 항목과 대응을 고정한다. 다른 worktree 쓰기·Claude 소켓은 더 좁게 제한한다. pnpm 부모 임시 쓰기 대신 고정 저장소를 쓴다. 상대 denyWrite와 홈 denyRead glob도 반영한다. 새 미분류 항목은 config 검사가 실패한다.

새 가드는 먼저 스테이징하고 해시 쓰기 모드로 생성한다. .DS_Store 등 미추적 파일은 제외한다. 기존 셸 예제의 허용 7건은 거부로 강화했다. 기존 거부를 허용으로 바꾼 것은 없다.


## 수정 3차 결정과 실측

대상은 master 7681ec8이다. Codex CLI 0.168.0을 썼다. 세 지적은 재현 테스트에서 모두 실패했고 수정 뒤 통과했다.

| 지적 | 사실과 조치 | 재현 테스트 |
|---|---|---|
| 1 | 어댑터에 workdir가 남은 입력은 범위 검사를 빠뜨렸다. 최상위 cwd와 tool_input·action의 작업 폴더를 각각 실제 경로로 검증한다. 명시한 폴더로 셸·패치·린트를 판정한다. 문자열 아님·빈 값·작업 사본 밖·서로 다른 인자는 거부한다 | R3 shell workdir fields cannot escape or override the directory used by the guard |
| 2 | 원격 경로만 strategy.branch 일치를 빠뜨렸다. 두 경로에서 문자열·브리프 일치와 기존 branchProblem을 함께 검사한다 | R3 remote and local launch branches must match a valid brief branch |
| 3 | 사용자 임시 폴더 전체의 Unix 소켓을 허용했다. worktree 실제 경로 해시와 UID로 정한 짧은 전용 폴더만 허용한다. 셸 TMPDIR와 종료 검사 TMPDIR가 같다 | R3 setup and hook checks share a dedicated temporary directory rather than the user temp root |

### 작업 폴더 입력

임시 CODEX_HOME과 로컬 가짜 Responses 서버를 썼다. 실제 모델·로그인·자격 증명 없이 설치 CLI가 받은 도구 정의와 PreToolUse 원문을 관찰했다. 헤드리스 호출은 실패한 계약 탐색을 포함해 5회다. 처음 두 호출은 응답 사용량 형식 오류, 셋째는 도구 목록 탐색이었다. 넷째에서 실제 셸을 실행했고 다섯째의 legacy 설정에서는 code-mode host가 비활성이라 실행되지 않았다.

| 노출·시도한 도구 | 도구 인자 | 실제 훅 |
|---|---|---|
| functions.exec 안의 tools.exec_command | cmd=pwd, workdir=세션/child | tool_name=Bash, cwd=세션/child, tool_input={command:pwd}. workdir는 tool_input에 남지 않음 |
| shell·shell_command·local_shell | 이번 세션의 도구 정의에 미노출 | 호출·필드 계약 미확인. local_shell은 기본 거부 유지 |
| 어댑터 입력 예제의 Bash·exec_command·shell_command·shell | workdir·cwd·working_directory·workingDirectory·working_dir·workingDir·work_dir·workDir·directory, action 안의 같은 필드 | 문자열·실제 경로·세션 범위를 독립 검증. 상대 경로는 훅 프로세스의 세션 폴더 기준 |

최상위 cwd로 정규화되는 현재 네이티브 호출은 기존 검사도 거부했다. 이번 수정은 정규화되지 않은 입력에서도 같은 경계를 강제한다. 역할은 계속 훅 프로세스의 세션 폴더에서 정한다.

### 전용 검사 임시 폴더

macOS에서는 /private/tmp/stock-check-<uid>-<worktree 해시>를 쓴다. Unix 소켓 경로 길이 제한 때문에 긴 worktree·사용자 임시 폴더 아래에 두지 않는다. 한 worktree는 하나의 T3 스레드가 소유하고 수정 회차는 같은 폴더를 쓴다. setup과 훅 진입에서 소유자·0700 권한·심볼릭 링크 아님을 확인한다. Stop 뒤와 회귀 정리 때 지운다. 비정상 종료로 남은 폴더는 세션을 닫은 뒤 사용자가 지운다. 새 생성물과 환경 설정은 저장소에 넣지 않는다.

운영 생성 프로필을 임시 CODEX_HOME에만 넣고 codex sandbox로 모델 없이 확인했다. 전용 폴더의 AF_UNIX bind·listen은 성공했고, 그 밖의 임시 폴더 소켓은 EPERM이었다. 테스트는 TMPDIR와 checkPlaces의 일치, 폴더 권한·심볼릭 링크 치환 거부도 검사한다. 사용자는 기존 컴퓨터별 프로필을 just codex-setup 출력으로 다시 만들어 새 세션에 적용한다.

### 메인의 병합 뒤 적용 확인: 2026-11-13

다음은 메인이 새 T3 Codex 세션에서 확인해 전달한 결과다. 이번 구현 세션의 재실측으로 바꾸어 쓰지 않는다.

| 시도 | 메인이 확인한 결과 |
|---|---|
| git -c core.hooksPath=/tmp status | 실행됨. 훅 미적용 |
| 원본 workdir의 pwd | 실행됨. 세션 범위 가드 미적용 |
| harness/.h41-profile-probe 생성 | 성공. 보호 경로 프로필 미적용 |
| CODEX_SANDBOX | 빈 값 |
| 사용자 Codex 프로젝트 신뢰 | 이 프로젝트의 신뢰 항목 없음 |

프로젝트 신뢰 등록과 훅 정의 신뢰는 사용자가 한다. 생성한 세션 프로필도 선택한 뒤 위 1분 절차를 새 T3 세션에서 반복한다. 신뢰 누락만이 원인이라고 단정하지 않는다. T3의 full-access가 프로필을 덮는지도 다시 확인한다.

### 회귀의 홈 격리

수정 1차 실험이 사용자 설정에 …/target/h41-probe/session 신뢰 항목을 남겼다는 메인의 확인을 반영한다. 홈 설정을 바꾸지 않았다는 당시 보고는 틀렸다. 사용자 설정 자체는 이번 작업에서 읽거나 고치지 않는다. 남은 항목은 사용자가 확인해 제거한다.

--ignore-user-config와 --ephemeral만으로 홈 쓰기를 격리하지 않는다. 회귀 실행기는 명시한 CODEX_HOME이 비공개 임시 폴더인지 먼저 검사하고 없으면 모델 실행 전에 실패한다. 기본 홈·상대 경로·심볼릭 링크·다른 소유자·그룹 및 전체 공개 권한은 허용하지 않는다. 로그인은 사용자가 별도로 한다. 실제 홈의 auth.json을 읽거나 복사·링크하지 않는다. [공식 인증 문서](https://learn.chatgpt.com/docs/auth#credential-storage)의 file 저장 방식을 명시해 이 임시 폴더만 쓴다. 회귀 모델의 도구 샌드박스는 임시 CODEX_HOME 전체도 deny로 두어 그 안의 인증·신뢰 기록을 읽거나 바꾸지 못한다.

사용자가 수동 회귀를 실행할 때의 절차다. 종료하면 로그인과 신뢰 기록을 포함한 임시 홈을 지운다.

```sh
export CODEX_HOME="$(mktemp -d)"
trap 'rm -rf "$CODEX_HOME"' EXIT
codex -c 'cli_auth_credentials_store="file"' login
just codex-regression
```

에이전트의 무인 계약 실험은 같은 방식의 빈 임시 CODEX_HOME과 인증이 필요 없는 로컬 가짜 제공자를 쓴다. 실제 모델이 필요하면 사용자가 준비한 임시 홈만 전달받는다. 실험 자료와 회귀 사본은 종료 때 지운다.
