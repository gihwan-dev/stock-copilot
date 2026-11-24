# front3 묶음 결정 표

- 상태: 확정 (2026-11-25, 묶음 구현 담당이 정함. 경량 절차 ADR 0173)
- 범위: U-23·U-05는 front1 묶음이 master에 먼저 올렸으므로 master 쪽을 기준으로 두고 빠진 몫만 보탰다. `src-tauri`와 `crates/*`는 고치지 않았다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| U-23 | 이 묶음의 첫 구현(기능 폴더 `settings-basic`, 커밋 `f0732237`)은 master의 `features/settings`와 같은 기능이라 걷어냈다. 함께 넣었던 DS `Disclosure`의 `keepMounted`도 되돌렸다. 접힌 고급 안의 내용이 DOM에 남으면 master의 U-23 시험(접힌 상태에 "키를 넣으면 켜집니다"가 없어야 함)이 깨진다 | 두 설정 화면 공존, 접혀도 DOM에 남기기 |
| U-23 | front2의 네 기능 폴더(`settings-system`·`-ai`·`-news`·`-principles`)는 master 배치대로 설정 본문 아래에 둔다. 고급 안으로 옮기면 front2 화면 시험(고급을 열지 않고 `/settings`에서 바로 찾음, 코어 설정 응답 없이 그려짐)을 고쳐야 해서 기존 시험 변경 금지에 걸린다. 사용자 서명 커밋으로 시험을 고친 뒤 옮길 수 있다(메인 확인 필요) | 시험을 고치고 고급 안으로 옮기기 |
| U-05 | master의 `SettingsChanged`(설정 범주)는 `["settings"]`만 무효화해 front2 고급 섹션을 다시 읽지 않았다(ADR 0065 "설정을 전부 다시 읽음"과 어긋남). front2 설정 쿼리 키를 `shared/lib/query-keys.ts`의 `settingsKeys`(`["settings", …]` 접두어)로 옮겨 같은 무효화가 접두어로 모두 닿게 했다. 연결 상태·현재 IP 키는 설정이 아니라 그대로 둔다. 계좌 선택 키 `["account-selection"]`은 front2 고정 시험이 키를 직접 보므로 그대로 두고, 계좌 바뀜은 `AccountSelectionChanged`의 전체 무효화가 맡는다 | 이벤트 매핑에 키를 하나씩 추가 |
| P-3 | 채팅 로그인 문구는 master에서 이미 터미널 절차 없는 문구로 고쳐져 손대지 않았다 | - |
