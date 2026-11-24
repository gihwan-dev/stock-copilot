# C-11 `open_external` 명령 결정 표

- 상태: 초안 (C-11, ADR 0173 경량 절차)
- 관련: `docs/tasks.md` C-11, `docs/design.md` SEC-10·8장, `docs/architecture.md` 3.6, `docs/research/R-13-legal-and-terms.md` Q6, `docs/research/R-16-local-security.md` Q6, ADR 0024, ADR 0089, ADR 0107

## 문서가 정하지 않아 안전한 쪽(거부)으로 정한 항목

| 항목 | 정한 것 (기본 = 거부) | 대안 | 사용자 영향 |
|---|---|---|---|
| 고정 주소 목록 | 문서(`design.md` 8장, `R-13` Q6)에 명시된 `https://claude.ai/settings/data-privacy-controls` 한 개만 허용하고 나머지 후보는 모두 거부 | Claude Code 설치 안내, 토스 Open API 안내 등 추측 주소를 미리 허용 | 문서가 확정한 학습 설정 바로가기 외 고정 주소는 추후 명시적으로 추가하기 전까지 열리지 않음 |
| 스킴·호스트 대소문자 | 소문자 `https://`와 소문자 ASCII DNS 호스트만 허용. `HTTPS://`, `CLAUDE.AI` 등 대문자 변형은 거부 | 파싱 시 스킴·호스트를 소문자로 접어 일치 판정 | 정규화된 수집 주소(`NormalizedUrl`)와 고정 주소는 원래 소문자라 정상 동작하며, 대문자 속임 입력은 차단됨 |
| 명시적 포트(`:443` 포함) | `:443`을 포함한 모든 명시적 포트 거부 (ADR 0024 `NormalizedUrl`과 동일) | `:443`만 기본 HTTPS 포트로 정규화해 허용 | 기본 포트를 생략한 정규 수집·고정 주소만 열리고 포트 혼동 우회를 차단함 |
| 빈 경로(`https://example.com`)와 끝 슬래시 | 경로는 `/`로 시작해야 하며 빈 경로와 끝 슬래시 유무 자동 보정 금지(정확한 구조 일치만 허용) | 빈 경로를 `/`로 보정하거나 끝 `/`를 무시 | `NormalizedUrl`과 고정 주소가 항상 `/`로 시작하므로 정상 링크는 동작하고 접두·경로 변형은 차단됨 |
| 프래그먼트(`#...`)와 `utm_*` 추적 쿼리 | `#` 프래그먼트가 있거나 저장된 수집 주소와 쿼리가 다른 입력(`utm_*` 추가 포함)은 거부 | `#fragment`·`utm_*`를 제거한 뒤 비교 | 수집 목록에 저장된 정규 주소 그대로만 열려 쿼리·프래그먼트를 통한 정보 유출·변조를 차단함 |
| 퍼센트 인코딩 정규화 및 금지 바이트 | 대문자 16진수(`%XX`)만 허용하고 제어 문자(`%00`..`%1F`, `%7F`), 역슬래시(`%5C`), 경로 내 슬래시(`%2F`), 이중 인코딩된 점·구분자·제어 문자(`%252E`, `%252F`, `%255C`, `%2500`)는 거부 | 소문자 `%xx` 허용 또는 경로 `%2F` 허용 | 비정규·이중 인코딩 우회와 경로 혼동을 차단. 경로에 `%2F`가 꼭 필요한 출처가 생기면 별도 결정 필요 |
| 경로 중복 슬래시(`//`)·점 경로·경로 내 스킴 | `//`, `.`/`..`(원시 및 `%2E` 복호화 포함), 경로 세그먼트의 스킴(`http:`, `https:`, `javascript:`, `data:`, `file:` 등) 거부 | 경로 문자열을 그대로 비교만 수행 | 프로토콜 상대 혼동과 경로 순회 우회를 파서 단계에서 차단함 |
| 쿼리 값의 오픈 리다이렉트 대상 | 쿼리 값(원시·1회·2회 퍼센트 복호화 기준)에 `://`, 선두 `//`, `\`, 위험 스킴 접두(`http:`, `https:`, `javascript:`, `data:`, `file:`, `vbscript:`, `blob:`, `about:`)가 있으면 수집 목록에 들어 있더라도 거부 | 수집 목록 일치만 확인하고 쿼리 내용은 검사하지 않음 | 수집 목록 오염이나 중계 경로를 악용한 외부 리다이렉트를 차단. Google News RSS(`/rss/articles/...?hl=ko&gl=KR`)처럼 쿼리에 외부 URL이 없는 중계 주소는 수집 목록 일치 시 허용 |
| `http://` 수집 항목 처리 | 수집 목록에 `http://` 주소가 있어도 `open_external`은 `http://`를 거부하며 `https://`로 자동 승격하지 않음 | `http://` 항목을 `https://`로 자동 치환해 열기 | SEC-10(`https`만 엶) 엄수. `http://` 전용 출처 링크는 열리지 않음 |
| 수집 목록 포트·실제 OS 열기 어댑터 제품 기본 연결 | `CollectedUrlCatalog`와 `ExternalUrlOpener`를 `application` 포트로 정의하고 `testkit-ports`에 가짜 구현을 둠. 제품 조립 기본값은 `Unavailable`(닫힌 실패: `retryable`)로 두고 실제 브라우저를 실행하지 않음 | `std::process::Command("/usr/bin/open")` 또는 `NSWorkspace`(unsafe FFI) 직접 연결 | 실제 SQLite 수집 목록 연결(S-05/C-05)과 macOS 열기 어댑터 결정 전까지 제품 조립은 임의 외부 실행 없이 안전하게 실패 |
| 허용 거부 IPC 오류 분류 | 비허용·파싱 거부 URL은 기존 닫힌 `IpcErrorKind::Bug`(`RequestRejection::MalformedRequest`)로 반환하고, 카탈로그 불일치는 `Bug`(`RepositoryIntegrity`), 열기 권한 거부는 `UserFixable`, 미연결·실행 실패는 `Retryable`로 분류 | 새 `IpcErrorKind::Blocked` 변형 추가 | 기존 U-02 IPC 오류 계약(`user-fixable`, `retryable`, `bug`, `no-data`)을 깨지 않고 내부 URL·사유 원문을 노출하지 않음 |

## 인계

| 대상 | 내용 |
|---|---|
| S-05 / C-05 | SQLite 정보 저장소(뉴스·공시·웹 수집 항목)에 `application::CollectedUrlCatalog` 구현을 연결해 `ExternalLinkRuntime`에 주입 |
| 플랫폼 어댑터(후속 결정) | `application::ExternalUrlOpener`의 실제 macOS 구현 방식(`/usr/bin/open` 프로세스 실행 허용 여부 vs `platform-macos`의 `NSWorkspace` 래퍼) 확정 및 `main.rs` 연결 |
| C-05 / AI-10 | 화면의 마크다운 링크(`DS-18` `Markdown.onOpenLink`)와 학습 설정 바로가기 버튼에서 `shared/ipc`의 `openExternal` 호출 연결 |
