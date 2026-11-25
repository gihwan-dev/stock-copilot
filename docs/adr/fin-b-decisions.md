# fin-b 묶음 설계 결정 기록

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| S-20 | `sqlparser`는 `=0.64.0`(잠금 파일에 없어 최신 안정판), 기본 기능을 끄고 `std`만 켠다. `visitor` 기능은 `sqlparser_derive` 의존을 더하므로 쓰지 않고 낱말 수 비교로 허용 위치를 판정한다. 자세한 규칙과 마이그레이션 잠금 절차는 `s-20-sql-structure.md` | `visitor` 기능으로 AST 전체 방문 |
| S-20 | 마이그레이션 상수는 `migrations/*.sql`과 글자까지 같은지로 가린다. 마이그레이션에 DML(데이터 옮기기)이 있어도 허용한다 | `MIGRATIONS` 배열을 구문 분석해 별칭 해석 |
| H-68 | 변이 대상은 파일 단위 경로로 적고(하위 폴더가 있는 모듈은 `<모듈>/**/*.rs`도 함께), 고정 테스트 `MUTATION_TARGET_GLOBS`를 같은 목록으로 갱신했다. 자세한 것은 `h-68-check-time.md` | 크레이트 전체 유지, 경로 패턴 규칙만 검사 |
| H-67 | 보관·병합·복원 한 묶음 명령(`staged-sync.mjs sync`)과 git 폴더의 막힘 기록(`staged-sync.mjs blocked`)으로 정했다. 자세한 것은 `h-67-staged-sync-and-blocked-exit.md`. 훅 해시 갱신은 사용자 몫으로 남겼다 | 승인 기준을 커밋 시작 시점에 고정 |
| H-26 | 새로 만든 것 없음. `signatures.yml`, `harness/ci/signatures/*`(기준 커밋의 서명자 목록·허용 목록·검증 스크립트 사용, 같은 푸시의 서명자 추가 무효, 기존 테스트 변경, 의존성 절 밖 변경)와 테스트 30개가 이미 master에 있고 통과한다. 남은 것은 서명 키(H-28), `allowed_signers`·`lock-point.txt` 기록, 규칙 세트로 모두 사용자 몫이다 | 브랜치 푸시 실패에도 이슈 알림(ADR 0020이 버림) |
| H-35 | 새로 만든 것 없음. 엄격 샌드박스 설정은 `harness/sandbox/policy.json`(`enabled`, `failIfUnavailable`, `allowUnsandboxedCommands: false`, 쓰기·읽기 거부, 네트워크 허용 목록)과 `lock-fragment.json`으로 이미 있고, 켜는 것은 H-20 절차다. `PATH` 앞 실행 파일 폴더는 샌드박스 기본 쓰기 범위(프로젝트와 `allowWrite`) 밖이라 따로 적지 않았다(적으면 Codex 정책 대응표가 실패한다). `node_modules/`·`target/` 서드파티 산출물 쓰기 차단과 S-03은 샌드박스 안 설치·빌드 실측이 필요해 건너뛰었다. `lock-fragment.json`이 지금 허용 목록과 어긋나 있으나 `fragment.mjs --write`는 사람 서명 커밋 전용이다 | `policy.json`에 홈·시스템 실행 파일 폴더를 `denyWrite`로 추가 |
| H-20 | 건너뛰었다. ADR 0018의 절차(`prelock.mjs` → `fragment.mjs --locked-settings` → `lock.txt` 켜기 → `hashes.mjs --write` → `settings.test.mjs` 기대값 변경 → `git commit -S`) 가운데 쓰기 모드 두 개를 세션 훅이 에이전트에게 거부하고(ADR 0018, 사람 서명 커밋 전용), 서명 커밋은 에이전트가 만들 수 없다. `lock.txt`만 켜면 해시가 서명 커밋 값과 달라 모든 세션의 훅이 닫힌 실패를 내고, 잠금 뒤에는 묶음 결정 기록(`docs/adr/*-decisions.md`) 추가도 거부되어 다른 스레드를 막는다 | `lock.txt`·`.claude/settings.json`을 손으로 고쳐 커밋 |
| H-68 | (2) 빌드 결과 공유와 전·후 시간 측정은 실측 항목이라 건너뛰었다. 변이 수만 `cargo mutants --list`로 쟀다 | 이 기기에서 씨앗·공유 캐시 재측정 |
