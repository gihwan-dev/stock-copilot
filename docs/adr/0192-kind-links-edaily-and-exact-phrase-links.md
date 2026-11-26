# ADR 0191: KIND 링크 복구, 이데일리 제외, Google 정확 구절 연결, 진단 범위

- 상태: 확정 (2026-11-26). v0.5.7 실설치 진단 기록을 보고 정한다. [ADR 0190](0190-news-retention-dart-sec-and-backoff.md)을 잇는다.

## 배경

v0.5.7 첫 회차(20:24 KST) 기록:

- KIND: `source-rejected detail="stage=items shape=xml items=500 kept=0 skipped=bad-url:500 ct=text/xml? charset=UTF-8"`. 응답은 정상 RSS였고 접수 번호도 모두 읽혔지만, 500건 전부 링크가 `NormalizedUrl` 검사에서 떨어졌다. 같은 회차에 DART RSS 대체가 50건을 저장했다.
- 이데일리: `source-call … transport=connect ms=8`. HTTP 응답 이전에 연결이 실패한다.
- 언론사 피드: 재시작 직후라 `known=전량 stored=0`. 재저장 방지는 동작한다.
- 연결 진단: 새로 저장한 기사만 세서 모두 0이었다. Google 뉴스는 새로 저장한 5건 모두 제목에 종목명이 없었다(`name_seen=0`).

## 결정

1. **KIND 링크.** `NormalizedUrl`이 받지 않는 문자(앞뒤 공백·줄바꿈, 공백, 한글 등)는 퍼센트 인코딩해서 다시 검사한다. 그래도 안 되면 읽은 접수 번호로 KIND 공시 뷰어 주소(`https://kind.krx.co.kr/common/disclsviewer.do?method=search&acptno=<번호>`)를 만든다. 진단에는 첫 항목 링크의 모양(스킴, 호스트, 경로, 쿼리 키 이름, 문제 문자 종류)만 남기고 값은 남기지 않는다.
2. **이데일리 제외.** 연결부터 실패해 기본 수집 목록과 설정 화면 목록에서 뺐다. 어댑터와 저장 태그는 남겨 예전 기사 읽기와 설정 복원이 깨지지 않게 한다.
3. **대체 언론사는 이번에 넣지 않는다.** 공개 RSS 안내가 확인된 후보는 세계파이낸스 증권 RSS(`https://www.segyefn.com/views/rss/stock.xml`, segye.com/rssInfo)다. 그런데 새 소스를 넣으려면 `source_statuses` 표의 `CHECK (source IN …)` 제약을 바꾸는 마이그레이션이 필요하다. `migrations.lock`은 보호 파일이라 사용자 서명 커밋으로만 바꿀 수 있다. 그동안 이데일리 기사는 종목별 Google 뉴스 검색으로 들어온다.
4. **Google 정확 구절 연결.** 세 글자 이상 이름은 따옴표로 묶어 정확한 구절로 검색하고, 그 결과를 검색한 종목에 주소 힌트로 바로 잇는다. 정확 구절 검색 결과는 본문에 그 이름이 들어 있으므로 제목에 이름이 없어도 그 종목 기사다. 두 글자 이름(예: 애플)은 일반 명사일 수 있어 지금처럼 제목 매칭만 쓴다.
5. **진단 범위.** `name_seen`, `matched`(제목만으로 연결되는 수), `needs_context`, `group_only`를 새로 저장한 기사가 아니라 그 회차에 받은 기사 전체로 센다. 재시작 뒤처럼 대부분이 `known`이어도 매칭 품질을 볼 수 있다.

## 다른 선택지

- 이데일리 주소 추측: 확인 수단 없이 바꾸면 다른 실패를 숨긴다.
- 이데일리 태그 자리에 다른 언론사를 넣기: 출처가 화면과 AI에 잘못 표시된다.
- Google 결과를 모두 검색 종목에 연결하기: 짧은 이름은 다른 뜻의 기사를 끌고 온다.
