# ADR 0194: 연결 실패를 종류별로 남기고, 이 기기에서 닿지 않는 소스를 설정에 표시한다

- 상태: 확정 (2026-11-26). v0.5.10 실설치 기록을 보고 정한다. [ADR 0192](0192-kind-links-edaily-and-exact-phrase-links.md)를 잇는다.

## 배경

v0.5.10에서 세계파이낸스도 이데일리처럼 `transport=connect`(91ms)로 HTTP 이전에 끊겼다. 두 언론사 모두 수십 ms 만에 끊기므로 이 맥의 보안 에이전트나 네트워크 필터가 특정 호스트를 막을 가능성이 크다. 지금 기록으로는 DNS 실패, TCP 거부, TLS 실패, 로컬 차단을 구분할 수 없다.

같은 회차의 연결 진단은 다음과 같았다.

| 소스 | name_seen | matched | needs_context | group_only |
|---|---|---|---|---|
| Google 뉴스 | 30 | 29 | 0 | 0 |
| 매경 | 1 | 1 | 0 | 0 |
| 연합 | 2 | 2 | 0 | 0 |
| KIND | 7 | 3 | 0 | 0 |

KIND의 나머지 4건은 문맥이나 그룹명이 아니라 경계 규칙에서 걸렸다. `삼성전자서비스`처럼 다른 회사 이름 안에 종목명이 들어간 경우로, 잇지 않는 것이 맞다. 그래서 매칭 규칙은 바꾸지 않는다.

## 결정

1. **연결 실패 종류.** `TransportFailure`에 `Dns`, `Refused`, `Reset`, `Tls`, `Blocked`를 더한다. 수집 HTTP 오류의 원인 사슬을 끝까지 따라가 가장 안쪽 원인으로 고른다.
   - io 오류 종류: `ConnectionRefused`는 tcp-refused, `ConnectionReset`·`ConnectionAborted`는 tcp-reset, `PermissionDenied`·`NetworkUnreachable`·`HostUnreachable`은 blocked-locally로 본다. macOS 네트워크 필터는 EPERM으로 끊는다.
   - 오류 문구: `dns error`, `failed to lookup`은 dns, `certificate`·`handshake`·`tls`는 tls로 본다.
   - 클라이언트는 프록시를 쓰지 않으므로(`no_proxy`) 프록시 경우는 없다. 분류되지 않으면 지금처럼 `connect`다.
2. **설정 표시.** 소스 상태 기록에서 "한 번도 성공한 적 없이 실패 중"인 소스를 `unreachable` 목록으로 보내고, 설정의 수집 소스 행에 "이 기기에서 연결되지 않아요. 잠시 쉬었다가 다시 시도해요. 종목 기사는 Google 뉴스 검색으로 계속 받아요."를 한 줄 띄운다. 한 번이라도 성공한 소스는 일시 실패로 보고 표시하지 않는다. 시도 간격은 ADR 0190의 실패 후퇴(3회 연속 실패 뒤 6시간)를 그대로 따른다.
3. **세계파이낸스는 목록에 남긴다.** 로컬 차단인지는 다음 설치본의 `transport=` 값으로 확정한다. 실패 후퇴 덕분에 비용이 거의 없고, 다른 사용자의 기기에서는 될 수 있다. `blocked-locally`가 확인되면 이 기기에서만의 문제이므로 기본 목록에서 빼지 않는다. `dns`나 `tls`로 나오면 주소나 인증서 문제이므로 그때 뺀다.

## 다른 선택지

- 연결 안 되는 언론사를 계속 바꿔 보기: 기기 필터라면 어느 언론사든 같을 수 있다. 종목별 기사는 Google 뉴스가 이미 가져온다.
- 소스 행에 상태 필드 넣기: 화면이 행을 그대로 저장 요청에 쓰므로 서버의 `deny_unknown_fields`에 걸린다. 응답에 따로 목록을 둔다.
