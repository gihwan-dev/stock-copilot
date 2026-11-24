# chat2 묶음 결정 표

경량 절차(ADR 0173)로 `chat2` 묶음(J-07, E-11)을 구현하며 문서가 정하지 않은 것을 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| J-07 | 코어 IPC `get_briefings`(`scope: "recent"`만)를 더했다. `BriefingRepository::recent()`를 감싼 `BriefingIpcState`(객체 안전 `BriefingHistory` 포트)를 Tauri 상태로 받고, 상태가 없으면 `no-data`를 돌려준다. 제품 조립은 아직 저장소를 넣지 않는다(다른 저장소 포트도 모두 `Unavailable`이라 같은 처지). 응답은 완료 시각 내림차순으로 가장 최근 하나를 `latest`, 나머지 최대 30개를 `past`로 보낸다. 완료 시각은 KST(`+09:00`) ISO 문자열로 바꿔 보낸다 | `PortTypes`에 `BriefingRepository` 추가(모든 조립 변경), 화면 원본만 두고 IPC 미추가 |
| J-07 | 실패 사유는 화면용 네 가지(`ai-unavailable`·`rate-limited`·`empty`·`rejected`)로 줄인다. 격리 거부와 제공자 없음은 모두 "AI를 쓸 수 없어"로 보인다(내부 장치 이름을 드러내지 않음, P-원칙). 출처는 종류(웹·뉴스·공시)와 제목만 보이고 웹은 "미검증 웹 값" 배지, 출처가 없으면 "출처 없음" | 내부 실패 이름 그대로 표시 |
| J-07 | 브리핑 카드는 주 화면 신호 카드 위에 둔다. 지난 브리핑은 거래내역처럼 현황 자리에 열리는 보조 경로 `/briefings`(종목 목록·채팅 유지, design 3장)이며, 지난 기록이 있을 때만 카드에 "지난 브리핑 보기"를 붙인다. AI 글은 DS `Markdown`으로 그리고 링크는 그 브리핑의 출처 주소만 `open_external`로 연다 | 카드 안에 접힌 목록, 별도 탭 |
| J-07 | 실제 앱은 `App`의 기본 원본(`sources`를 주지 않을 때)에서 `pendingScreenSources`에 `briefings: getBriefings`만 바꿔 끼운다(`main.tsx`는 `<App />` 그대로, DS09 부트스트랩 시험). `BriefingFinished` UI 이벤트를 `briefing` 범주로 해석해 `["briefings"]` 쿼리를 무효화한다(이전에는 해석하지 못해 버려졌다) | 모든 쿼리 무효화 |
| J-07 | `call.ts`의 250줄 상한에 맞추려고 `openExternal`·`setOrderPermission`을 같은 동작(시간 제한·거절→`framework`)의 `raceTimeout` 형태로 줄이고 `getBriefings`를 같은 파일에 두었다 | 새 호출 어댑터 파일(거절 처리 허용 목록 변경 필요) |
| J-07 | 제품 브리핑 읽기는 `SqliteRepositories`의 `BriefingRepository`로 하고 조립 루트(`main.rs`)가 `BriefingIpcState::from_repository`로 넣는다. 같은 시각 native2가 `SqliteRepositories`를 `store-sqlite`로 옮기며(arch-check `adapter-construction`) 이 구현과 시험도 함께 옮겼으므로 합류 때 master 쪽을 받았다. 데이터베이스가 열리기 전에는 저장소 오류로 화면이 "불러오지 못했어요"를 보인다 | 조립 루트 전용 읽기 어댑터 |
| E-11 | 새 신호 → macOS 알림 경로(`SignalRaised` → `NotificationEventSubscriber` → `FilteredNotifier` → `NotificationDelivery`)는 U-14와 native2가 이미 만들고 제품 이벤트 버스에 연결했다. 이 묶음은 `product_event_bus`를 송출 포트에 대해 제네릭으로 바꿔, 제품 조립 그대로 가짜 송출을 넣고 새 신호 하나가 "새 매매 신호" 알림 하나가 되는지, 신호 해제는 알리지 않는지, 꺼 둔 알림은 보내지 않는지 시험한다. 실제 배너 송출(`tauri-plugin-notification`)은 잠금 파일에 없어 새 패키지 금지로 넣지 않았고 제품 송출은 계속 `UnavailableNotificationDelivery`다 | `objc2-user-notifications` 직접 호출(문서가 정하지 않은 `unsafe`), 플러그인 추가 |
