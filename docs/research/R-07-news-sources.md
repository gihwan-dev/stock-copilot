# R-07 뉴스·공시 소스의 조건과 수집 기법

- 조사일: 2026-11-11 (KST). 따로 적지 않았으면 모든 "확인"은 이날 한 것이다.
- 실측 환경: macOS, `curl`, Python 3 표준 라이브러리. 요청 머리말은 일반 브라우저형 `User-Agent`(`Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) rss-check`)만 썼고 개인 정보는 보내지 않았다. DART·네이버·FRED 키는 쓰지 않았다. SEC EDGAR 데이터 API는 호출하지 않았다(문서만 읽음).
- 실험 스크립트와 받은 원본은 `/private/tmp/claude-501/-Users-choegihwan--t3-worktrees-sure-t3-7f34ca39-5542c437df4f301a/d672a99d-9dd8-4133-b4ab-9b52494be3de/scratchpad/research/R-07/`에 있다(저장소 밖).
- 법령 해석은 조사 결과일 뿐 법률 자문이 아니다. 최종 판단은 R-13에서 한다.

## 1. 요약

1. **네이버 검색 API는 설계 그대로 쓸 수 없다.** 개발자센터는 2026-07-30 24:00부터 검색 API 신규 신청을 받지 않는다. 신규 사용자는 NAVER API HUB(네이버클라우드)로 가야 한다. 두 약관 모두 결과를 허용 범위 밖에서 저장(캐시 포함)·가공하지 못하게 하고, 개발자센터 약관은 기기 보관을 "24시간 또는 새 질의 시까지"로 묶는다. 영구 저장하고 묶음·매칭하는 지금 설계와 충돌한다.
2. **국내 공시의 빠른 경로는 KIND RSS다.** 키 없이 `currentPageSize=500`이면 하루치 전체(실측 478건, 지분공시 포함)를 분 단위 시각과 함께 준다. DART `list.json`은 날짜(`rcept_dt`)만 주고 시각은 주지 않는다. 그래서 2분 감시는 KIND로 하고, DART는 종목 코드·고유번호·정정 정보를 보강하는 데 쓴다. DART에도 키 없는 공식 RSS(최근 50건, 지분·펀드 제외)가 있다.
3. 언론사 RSS 넷(한경·매경·연합·이데일리)과 한국은행·연준 RSS는 모두 200으로 응답했다. 다만 조건과 품질이 제각각이다. 연합뉴스는 "비상업적 블로그와 개인적 용도"만 허용하고, 기사에 "AI 학습 및 활용 금지"를 붙인다. 한경 피드는 CDN 캐시 때문에 약 55분 늦었다. 조건부 GET(304)은 이데일리·한경·연준만 됐다.
4. Google 뉴스 RSS는 피드 안에 "개인 피드 리더에서 개인적·비상업적으로 표시하는 목적에 한함, 그 밖의 이용은 명시적으로 금지"라고 적혀 있다. 링크도 원문이 아닌 Google 중계 주소다. Yahoo RSS는 문서가 없는 비공식 엔드포인트이고 curl 기본 UA로는 404가 난다. 둘 다 보조로만 쓴다.
5. 종목 매칭과 묶음은 로컬 규칙으로 충분히 가볍게 된다(390개 제목에 0.2초 미만). 단, 단순 부분 문자열 매칭이나 단일 연결 군집은 실측에서 오탐을 냈다("채비", "대상", "SG"⊂"ESG", LG이노텍과 삼성전자 기사가 한 묶음). 그래서 경계·조사 규칙, 불용 종목명 목록, 리더(대표) 기준 군집이 필요하다. 미국 지수·금리·원자재·달러는 토스 미국 ETF 시세, 미 재무부 금리 XML, FRED API(키 필요)로 채운다.

## 2. 질문별 답

### Q1. 소스별 조건

실측 시각은 2026-11-11 17:20~17:25 KST다. "최신 항목 지연"은 받은 시각과 피드 맨 위 항목 시각의 차이다. 기사 발행 후 피드에 실리기까지의 지연이 아니라 그 시점의 신선도를 보여 주는 값이다.

| 소스 | 주소 (실측) | 응답 | 항목 수 / 덮는 기간 | 필드 | 조건부 GET | 최신 항목 지연 |
|---|---|---|---|---|---|---|
| KRX KIND 오늘의 공시 | `https://kind.krx.co.kr/disclosure/rsstodaydistribute.do?method=searchRssTodayDistribute&repIsuSrtCd=&mktTpCd=0&searchCorpName=&currentPageSize=100` | 200 `text/xml` | 15/100/500 지정 가능. 500이면 478건(07:49~17:19, 하루 전체) | title(`[유]회사명 공시명`), link(`acptNo` 14자리), pubDate(+0900, 분 단위), author(회사명), category(수시·지분·정기·공정공시 등) | 없음 (`no-cache`, Akamai 봇 관리 쿠키 `bm_s` 발급) | 약 4분 |
| DART 최근공시 RSS | `https://dart.fss.or.kr/api/todayRSS.xml` | 200 | 50건(약 32분 분량, 지분·펀드 제외) | title(`(유가)회사 - 보고서명`), link(`rcpNo`), category(시장), creator, dc:date(UTC) | 미확인 | 약 1분 |
| DART 회사별 RSS | `https://dart.fss.or.kr/api/companyRSS.xml?crpCd={corp_code}` | 200 | 최근 5영업일 | 위와 같음, 지분공시 포함 | 미확인 | - |
| DART Open API | `https://opendart.fss.or.kr/api/list.json` 등 | (키 없이 302) | `page_count` 최대 100 | 3장 Q2 참조 | 해당 없음 | 시각 필드 없음 |
| 한국경제 증권 | `https://www.hankyung.com/feed/finance` | 200 | 50건(약 11시간) | title, link, author, pubDate. **description 없음** | 304 됨 (`Last-Modified`) | **약 55분** (Cloudflare `Age: 2740`) |
| 매일경제 증권 | `https://www.mk.co.kr/rss/50200011/` | 200 | 50건(약 24시간) | no(기사번호), title, link, category, author, pubDate, description(약 100자), media:content | 없음 (`no-store`) | 약 43분 |
| 연합뉴스 마켓+ | `https://www.yna.co.kr/rss/market.xml` | 200 | 120건(약 9시간) | title, link, guid, pubDate, dc:creator, description(최대 약 100자), 이미지 | `ETag`가 요청마다 바뀌어 304 안 됨, `Expires` 약 2분 | 약 7분 |
| 연합뉴스 경제 | `https://www.yna.co.kr/rss/economy.xml` | 200 | 120건 | 같음 (마켓+와 URL 중복 있음) | 같음 | 약 10분 |
| 이데일리 증권 | `http://rss.edaily.co.kr/stock_news.xml` | 200 | 50건(약 8시간) | title, link, description(약 160자), category, author, pubDate | 304 됨 (`ETag`+`Last-Modified`) | 약 6분 |
| Google 뉴스 검색 | `https://news.google.com/rss/search?q={질의}&hl=ko&gl=KR&ceid=KR:ko` | 200 | 약 100건(최근 며칠) | title(`제목 - 언론사`), link(**Google 중계 주소**), guid, pubDate, source(언론사 URL) | 없음 (`no-store`) | - |
| Yahoo Finance 헤드라인 | `https://feeds.finance.yahoo.com/rss/2.0/headline?s=AAPL&region=US&lang=en-US` | 200 (브라우저형 UA). **curl 기본 UA는 404** | 약 20건. 쉼표로 여러 종목 가능, `005930.KS`도 응답(영문 기사) | title, link, guid, pubDate, description | `max-age=300` | - |
| 한국은행 보도자료(전체) | `https://www.bok.or.kr/portal/bbs/B0000552/news.rss?menuNo=200690` | 200 | 100건, 225KB (description에 HTML 본문 일부) | title, link, guid, pubDate, description | 없음 (`no-cache`) | - |
| 한국은행 보도자료(통화정책) | `https://www.bok.or.kr/portal/bbs/P0000559/news.rss?menuNo=200690` | 200 | 100건, 465KB | 같음 | 없음 | - |
| 연준 보도자료(전체·통화정책) | `https://www.federalreserve.gov/feeds/press_all.xml`, `.../press_monetary.xml` | 200 | 20건 / 15건 | title, link, guid, description, category, pubDate | 304 됨 | - |
| 미 BEA 발표 | `https://apps.bea.gov/rss/rss.xml` | 200 | 49건, `NextReleaseDate` 필드 포함 | - | 미확인 | - |
| 미 BLS | `https://www.bls.gov/feed/bls_latest.rss`, `.../schedule/news_release/bls.ics` | **403** (일반 UA 거부) | - | - | - | - |

**이용 조건, 한도, 인증**

| 소스 | 인증 | 한도 | 이용 조건 요지 (원문 근거) |
|---|---|---|---|
| DART Open API | 40자 인증키(무료). 1인 1키 (약관 제19조 ⑤) | 오류 코드 020 "요청 제한 초과, 일반적으로 20,000건 이상". 분당 한도는 공개되지 않음(미확인). 과도한 접속은 이용 제한 가능 (약관 제10조 ⑥) | 저작권과 이용은 저작권법·공공데이터법을 따른다 (제16조 ④). 저장·가공을 금지하는 조항은 없다. 정확성은 보장하지 않는다 (제23조) |
| DART RSS | 없음 | 미확인 | 별도 약관을 찾지 못함(미확인) |
| KIND RSS | 없음 | 미확인. Akamai 봇 관리를 쓰므로 과도한 호출은 차단될 수 있음(추정, 미확인) | KIND 이용약관 원문을 찾지 못함(미확인) |
| 네이버 검색(개발자센터) | Client ID/Secret 머리말 | 하루 25,000회 | **2026-07-30 24:00부터 신규 신청 중단**. 기존 이용자만 2027-06-30까지 쓸 수 있다. 검색 API 특약 2.3: 데이터(가공물·파생물 포함)를 복사·저장·캐싱하지 못한다. 2.4: 기기 보관은 "24시간 또는 새 질의 시까지 중 더 짧은 기간", 서버 이력 조회용 보관은 최대 21일. 2.2: 가공 없이 노출하고 출처와 원본 링크를 표시해야 한다 (2026-10-07 개정 약관) |
| 네이버 검색(NAVER API HUB) | `X-NCP-APIGW-API-KEY-ID`/`-KEY` | 검색 카테고리 통합 월 775,000건, 키당 50 RPS, 초과 시 429. 뉴스 문서에는 아직 "하루 25,000회"라고 적혀 있다(문서끼리 다름) | 한시적 무료이고 유료로 바뀌면 미리 공지한다. 약관 7.3 ③: 허용 범위를 넘는 복제, 저장(캐시 포함), 가공을 금지한다. 검색 특약 2.1: 독립적으로 노출하고 수정·변조를 금지한다 (2026-06-25 시행) |
| 연합뉴스 RSS | 없음 | 미확인 | "비상업적 블로그와 개인적인 용도로만 … 허용", "사전 서면 허가 없이 … 상업적으로 이용하는 것을 금지". 기사 하단에 "무단 전재-재배포, AI 학습 및 활용 금지" |
| 한국경제 RSS | 없음 | 미확인 | RSS 이용 조건 페이지를 찾지 못함(미확인). 기사 하단에 "사전 허가 없는 무단 전재·복제·배포·캡처 공유·AI 학습 활용 및 상업적 이용을 금합니다" |
| 매일경제 RSS | 없음 | 미확인 | RSS 페이지 하단에 "무단 전재, 재배포 및 AI학습 이용 금지". robots.txt가 GPTBot·ClaudeBot·anthropic-ai·CCBot 등을 전면 차단 |
| 이데일리 RSS | 없음 | 미확인 | RSS 이용 조건 페이지를 찾지 못함(미확인. `rss.edaily.co.kr/`은 403). 기사 하단에 "무단전재, 재배포 금지" |
| Google 뉴스 RSS | 없음 | 공개 한도 없음(미확인) | 피드 `<copyright>`: "made available solely for the purpose of rendering Google News results within a personal feed reader for personal, non-commercial use. Any other use of the feed is expressly prohibited." |
| Yahoo RSS | 없음 | 미확인 | Yahoo 약관의 RSS 조항은 피드 내용을 수정 없이 표시하고, 출처를 밝히고 원문으로 링크하라고 한다. 별도 조항은 허락 없는 자동 수집을 금지한다 |
| 한국은행 RSS | 없음 | 미확인 | 공공기관 자료. 별도 조건은 찾지 못함(미확인) |
| 연준 RSS | 없음 | 미확인 | 미 연방 정부 자료. 이용 조건은 확인하지 않음(미확인) |

### Q2. DART

- **시장 전체 공시 목록**: `GET https://opendart.fss.or.kr/api/list.json?crtfc_key=…&bgn_de=YYYYMMDD&end_de=YYYYMMDD&page_count=100&page_no=n`. `corp_code`를 빼면 시장 전체를 받지만 **조회 기간은 3개월로 제한**된다. `corp_cls`(Y·K·N·E)는 한 값만 받는다. 생략하면 전체다. `pblntf_ty`(A~J)와 `pblntf_detail_ty`로 유형을 거를 수 있다. 응답에는 `total_count`, `total_page`가 있다.
- **응답 필드**: `corp_cls, corp_name, corp_code, stock_code, report_nm, rcept_no(14자리), flr_nm, rcept_dt(YYYYMMDD), rm`. **접수 시각 필드가 없다.** 분 단위 시각이 필요하면 KIND RSS나 DART RSS를 써야 한다.
- **정정 공시**: `report_nm` 앞에 `[기재정정]`, `[첨부정정]`, `[첨부추가]`, `[변경등록]`, `[발행조건확정]` 등이 붙는다. 원래 공시의 `rm`에는 "정"(이후 정정 제출)과 "철"(철회) 표시가 붙는다. `last_reprt_at=Y`로 받으면 최종본만 온다. **정정본이 원 공시의 `rcept_no`를 가리키는 필드는 목록 API에 없다**(문서 기준). 그래서 원본과 정정본은 `(corp_code, 꼬리표를 뗀 report_nm)`으로 묶는 수밖에 없다. 이 방식의 정확도는 미확인이다.
- **고유번호 대응표**: `GET https://opendart.fss.or.kr/api/corpCode.xml?crtfc_key=…`가 ZIP(내부 XML)을 준다. 필드는 `corp_code(8), corp_name, corp_eng_name, stock_code(6, 상장사만), modify_date`이고 갱신 주기는 문서에 없다(미확인). 하루 한 번 받아 `modify_date`로 바뀐 것만 반영하는 방식을 권한다.
- **재무제표**: `fnlttSinglAcntAll.json`(단일회사 전체 재무제표). 필수 인자는 `corp_code, bsns_year(2015년 이후), reprt_code(11013 1분기, 11012 반기, 11014 3분기, 11011 사업), fs_div(OFS 별도, CFS 연결)`다. 응답은 `sj_div(BS, IS, CIS, CF, SCE), account_id(XBRL 표준 계정 ID, 비표준이면 대체 표시), account_nm, thstrm_amount, thstrm_add_amount(누적), frmtrm_*, bfefrmtrm_*, currency`이고 금액은 문자열이다. 분기·반기 손익의 `thstrm_amount`는 3개월치이고 누적은 `thstrm_add_amount`에 있다. 이 밖에 단일회사 주요계정, 다중회사 주요계정, 주요 재무지표, XBRL 원문 내려받기 API가 있다(OpenDART 메뉴로 확인, 세부는 미확인).
- **실측: KIND와 DART 번호의 관계.** 같은 시각에 받은 KIND 480건과 DART RSS 50건을 대조했다. 거래소를 거쳐 들어온 공시는 DART `rcpNo`의 9번째 자리가 `8`(유가)이나 `9`(코스닥)이고, KIND `acptNo`는 그 자리가 `0`이며 나머지는 같았다(23쌍. 예: DART `20261111800749` ↔ KIND `20261111000749` "후성 금전대여 결정"). 금감원에 직접 제출된 DART 공시(9번째 자리 `0`)는 KIND 번호와 숫자가 같아도 **다른 공시**였다(13건 전부 불일치). 따라서 번호를 그대로 같은 키로 쓰면 안 된다. 이 규칙은 하루치 표본에서 나온 것이라 공식 근거는 미확인이다.

### Q3. SEC EDGAR

- **요청 머리말**: 요청 머리말에 user agent를 밝혀야 한다. 문서 예시는 `User-Agent: Sample Company Name AdminContact@<sample company domain>.com`이다. 개인이라면 이름과 이메일이 들어가므로 **사용자가 정할 일**로 남긴다(설계 9장과 같음).
- **호출 한도**: "Current max request rate: 10 requests/second". SEC는 공정한 접근을 위해 더 낮출 수 있다.
- **데이터 API** (인증과 키 불필요, CORS 미지원):
  - 종목별 공시 이력: `https://data.sec.gov/submissions/CIK##########.json` (CIK는 앞을 0으로 채운 10자리). 이름, 거래소, 티커 메타데이터가 들어 있고 보통 1초 미만 지연으로 실시간 갱신된다.
  - 재무(XBRL): `.../api/xbrl/companyfacts/CIK##########.json`, `.../companyconcept/CIK…/us-gaap/{개념}.json`, `.../frames/us-gaap/{개념}/USD/CY2019Q1I.json`. 보통 1분 미만 지연.
  - 일괄 파일: `companyfacts.zip`, `submissions.zip`. 매일 새벽 3시(ET)쯤 다시 만든다.
  - 티커와 CIK 대응: `https://www.sec.gov/files/company_tickers.json`(EDGAR 검색창의 자동 완성용. 정확성은 보장하지 않음), `company_tickers_exchange.json`.
  - 최신 공시 피드: `/cgi-bin/browse-edgar?action=getcurrent&…&output=atom`. 회사(CIK)와 양식(8-K, 10-Q 등)으로 거를 수 있다. 갱신 주기는 문서에 없다(미확인).
- 이번 조사에서는 실제 호출을 하지 않았으므로 응답 형식과 지연은 실측하지 않았다(미확인).

### Q4. 더 나은 공식 소스

| 필요 | 소스 | 상태 |
|---|---|---|
| 국내 공시 저지연 | **KIND RSS**(위), **DART RSS** `todayRSS.xml` | 확인. DART RSS는 지분·펀드를 뺀 최근 50건뿐이어서 장 마감 직후 몰릴 때 32분 분량밖에 안 된다 |
| 종목 마스터(국내, 키 없이) | KIND 상장법인목록 내려받기 `https://kind.krx.co.kr/corpgeneral/corpList.do?method=download&searchType=13` | 확인. EUC-KR HTML 표(xls로 위장) 2,767개 종목. 회사명, 시장, 종목코드, 업종, 주요제품, 상장일. **76개 종목은 영숫자 코드**(예 `0153R0`) |
| 국내 실적 일정 | KIND 공시의 "기업설명회(IR) 개최(안내공시)"(하루 31건 관측), "영업(잠정)실적(공정공시)", "연결재무제표기준영업실적" | 확인(제목 분류로 뽑을 수 있음). 미래 실적 발표일만 모은 공식 달력 API는 못 찾음(미확인) |
| 미국 실적 일정 | 공식 무료 실적 달력은 못 찾음 | 미확인. 사후로는 SEC 8-K(실적 발표는 보통 Item 2.02, 문서로는 미확인)로 알 수 있다 |
| 미국 경제 지표 일정 | BEA RSS(`NextReleaseDate` 필드 있음, 확인), 연준 FOMC 일정 페이지(200 확인), BLS 일정 ics(**403**) | BLS는 일반 UA로 막힌다. 우회 방법은 미확인 |
| 국내 경제 지표 | 한국은행 ECOS API(`https://ecos.bok.or.kr/api/StatisticSearch/{키}/json/kr/…`) | 공개 `sample` 키로 기준금리(722Y001) 응답 확인. 정식 키와 한도는 미확인 |
| 증권사 리포트 요약 | 공식 API는 못 찾음 | 미확인. 네이버 금융 리서치나 한경 컨센서스를 긁어 오는 방식은 각 사이트 약관상 위험하므로 넣지 않기를 권한다 |
| KRX 시세·지수 일별 | KRX 정보데이터시스템 OPEN API(`openapi.krx.co.kr`, 인증키 신청제) | 페이지 응답만 확인. 한도와 지연은 미확인 |

### Q5. 저작권법과 약관에서의 위치

**법령** (저작권법 [시행 2026. 10. 29.] [법률 제21595호, 2026. 4. 28., 일부개정], 국가법령정보센터 원문)
- 제7조 제5호: "사실의 전달에 불과한 시사보도"는 보호받지 못한다. 대법원 2006. 9. 14. 선고 2004도5350 판결은 기사 가운데 단순한 사실 전달을 넘는 부분만 가려 침해를 판단해야 한다고 했다. 2007다354 판결은 정형적이고 간결한 사실 전달 기사의 저작물성을 부정했다(2차 출처에서 확인. 판결 원문은 미확인).
- 제30조(사적이용을 위한 복제): "영리를 목적으로 하지 아니하고 개인적으로 이용하거나 가정 및 이에 준하는 한정된 범위 안에서 이용하는 경우에는 그 이용자는 이를 복제할 수 있다."
- 제35조의5(공정한 이용): 이용 목적, 저작물의 종류, 이용 비중, 시장에 미치는 영향을 고려한다.
- **텍스트·데이터 마이닝(정보분석) 면책 조항은 현행법에 없다.** 같은 원문에서 "정보분석", "인공지능", "기계학습"을 검색했지만 나오지 않았다.

**해석 (미확정)**
- **제목, 요약, 링크를 개인 PC에 저장**: 제목은 짧은 사실 표현이라 저작물성이 약하다는 것이 일반적인 해석이다(판례로 직접 확인하지는 못함, 미확인). 요약(description)은 언론사 리드 문장이라 저작물일 수 있다. 다만 본인 1명의 비영리 개인 이용이면 제30조 범위에 들 여지가 크다. **법보다 약관과 피드 조건이 더 빡빡하다.**
  - 연합뉴스, Google: 개인적·비상업적 범위면 표면상 허용된다.
  - 네이버(두 약관 모두): **저장 자체가 원칙적으로 금지**된다. 개발자센터 약관 기준으로 기기에 24시간까지만 둘 수 있다.
- **AI가 분석 시점에 본문을 읽는 것**: 앱이 본문을 가져와 Claude Code·Codex CLI 프롬프트에 넣으면 본문이 AI 제공자 서버로 전송된다. 저장하지 않더라도 복제와 전송이 일어난다.
  - 연합뉴스("AI 학습 및 활용 금지"), 한경("AI 학습 활용"), 매경("AI학습 이용 금지")은 AI 이용을 명시적으로 막는 문구를 단다. 연합뉴스 문구는 학습뿐 아니라 "활용"까지 막는다.
  - 비회원 방문자에게 이 문구가 계약으로서 구속력이 있는지는 미확인이다.
  - 학습에 쓰지 않는 개인 분석이 제30조나 제35조의5로 정당화되는지도 판례를 찾지 못했다(미확인).
- **권고**:
  1. 본문은 저장하지 않는다(현행 설계 유지).
  2. 소스마다 `allow_ai_read` 설정을 둔다. 연합뉴스는 기본으로 끄고, 나머지도 처음에는 끄고 사용자가 켜게 한다.
  3. AI 분석은 기본적으로 제목, 요약, 공시(공공 자료)로 한다.
  4. 네이버 결과는 저장하지 않는다.

### Q6. 같은 사건의 기사 묶기 (로컬, 가벼운 방법)

**실험**: 한경·매경·연합(2)·이데일리 RSS와 Google 뉴스("삼성전자") 제목 498개를 썼다. 정규화는 `[특징주]`·`(종합)` 꼬리표, 기호, 공백 제거와 소문자화이고, 특징은 글자 2-gram 집합이다.

| 방법 | 결과 |
|---|---|
| URL 정규화(스킴, `www.`/`m.`, 추적 인자 제거) | 498개 중 456개가 고유 링크였다. 대부분 연합 마켓+와 경제 피드가 같은 기사를 실은 경우다. 같은 피드 안이나 피드끼리의 정확한 중복은 URL로 잡힌다. **Google 뉴스 링크는 `news.google.com/rss/articles/…` 중계 주소**다. curl로 열면 Google 안에서 302로 돌 뿐 원문으로 가지 않으므로, URL로 묶으려면 `<source url>`와 제목을 함께 써야 한다 |
| 2-gram Jaccard, 단일 연결(union-find), 임계값 0.45 | 쌍 비교 12만여 번에 **0.17초**. 묶음이 56개 생겼다. 그런데 가장 큰 묶음(42개)에 "LG이노텍, 10년 연속 동반성장 최우수"와 "삼성전자, 동반성장지수 15년 연속 최우수"가 같이 들어갔다. **사슬 효과로 다른 회사 기사가 섞인다** |
| 2-gram Jaccard, 리더(대표 기사) 비교, 0.45 | 묶음 373개, 여럿 모인 묶음 60개(크기 35, 8, 8, 6…). 가장 큰 묶음에 다른 회사 기사가 섞이지 않았다. "[속보] 美빅스텝 우려에도 급등…코스피 2.6%·코스닥 5.6%↑" 같은 언론사 간 같은 기사가 잘 묶였다 |
| 경계 사례(0.35~0.45) | "삼성전자, 동반성장지수 15년 연속 최우수"의 여러 표현, "코스피 2.6% 오른 5375.36 마감" vs "코스피 2.60% 상승 5,375선 마감" 등이 이 구간에 있었다. 같은 사건인데 표현이 갈리는 구간이다 |

**권고 순서**
1. 정규화한 URL이 같으면 같은 항목이다.
2. 같은 종목 집합이고 발행 48시간 안에서 2-gram Jaccard ≥ 0.45를 **리더와 비교**해 묶는다. 단일 연결은 쓰지 않는다.
3. 0.35~0.45 구간만 선택적으로 임베딩 코사인 유사도로 판정한다.

**임베딩 선택지**: `fastembed` crate 7.7.0(2026-11-09 갱신)이 ONNX로 `intfloat/multilingual-e5-small`·`-base`, `BAAI/bge-m3`, `paraphrase-multilingual-mpnet-base-v2`를 지원한다(README 확인). 기반 `ort`는 2.0.0-rc.15(정식판 없음). 한국어 제목에서 이 모델들의 정확도와 macOS에서의 속도는 미확인이다. PERF-5(대기 메모리 300MB 미만)를 생각하면 상주시키지 말고 필요할 때 띄워야 한다.

**기타**: `simhash` 0.3.0, `gaoya` 0.2.2(MinHash LSH)는 항목이 수만 개를 넘을 때나 필요하다. 하루 수천 건이면 시간 창 안에서 전부 비교해도 충분하다(실측 근거: 498개에 0.17초, Python).

### Q7. 한국어 제목에서 종목 찾기

**실험**: KIND 상장법인목록 2,767개 이름으로 위 390개 국내 제목을 매칭했다.

| 방법 | 결과 |
|---|---|
| 단순 부분 문자열(긴 이름 우선, 겹침 금지) | 127개 제목이 매칭됐다. 오탐: "SG"⊂"ESG"(5건), "DB"⊂"ADB", "TP"⊂"CPTPP", "레이"⊂"릴레이/디스플레이", "동서"⊂"동서발전", "삼영"⊂"강삼영", "농심"⊂"농심천심", "대상"(일반 명사), "채비"(일반 명사) |
| 경계 규칙 추가: 영문 이름은 영숫자 단어 경계, 한글 이름은 앞이 문장 시작·공백·괄호·따옴표·`·`이고 뒤가 조사(은·는·이·가·을·를·의·에·와·과·도·로·에서 등)·공백·구두점·숫자·끝 | 109개 제목이 매칭됐다. 남은 오탐은 "채비", "대상"(일반 명사), "SK"(SK에코플랜트는 비상장 계열사인데 상장사 "SK"로 잡힘). 반대로 "삼성전자&SK하이닉스"는 `&` 경계 누락으로 놓쳤다 |

**권고 규칙**
1. 사전: 토스 종목 마스터의 정식명 + 약칭 표(사용자가 편집. 예: "하이닉스"→SK하이닉스, "현대차"→현대자동차, "엔씨"→NC) + 영문명.
   - Rust는 `aho-corasick` 1.1.5로 한 번에 찾고, 위 경계·조사 규칙으로 거른다.
2. **일반 명사와 같은 종목명**(대상, 채비, 동서, 삼영, 레이, 한진처럼 짧거나 흔한 말) 목록을 둔다.
   - 이 목록의 이름은 제목 맨 앞이거나 바로 뒤에 `,`가 오거나 증권 문맥어(주가, 급등, 공시, 실적, 특징주)가 함께 있을 때만 인정한다.
   - 그 밖의 경우는 "애매" 등급으로 빠른 모델에 넘긴다(설계 5.1과 같음).
3. **그룹명 접두**(SK, LG, 롯데, 한화, 현대, 삼성, CJ, GS, DB)는 바로 뒤에 다른 글자가 붙으면(SK에코플랜트, LG이노텍) 지주사로 매칭하지 않는다.
   - 더 긴 상장 계열사명이 있으면 그쪽을 우선한다(긴 이름 우선).
   - 비상장 계열사면 "그룹 언급"으로만 표시한다.
4. 형태소 분석기 `lindera` 6.2.0(`embed-ko-dic` 기능)은 조사 분리에 쓸 수 있다. 하지만 사전에 없는 종목명(신규 상장)을 잘못 쪼갤 수 있으므로 주 매칭은 사전 매칭으로 하고, 형태소 분석은 보조로 둔다. 품질은 미확인이다.
5. 공시는 매칭할 필요가 없다. KIND는 `author`에 회사명을, DART는 `stock_code`를 준다. 단, KIND는 종목코드를 주지 않으므로 회사명→코드 대응표로 바꾼다. KIND 제목 앞의 `[유]`·`[코]`는 시장이다.
6. **종목 코드는 6자리 숫자라고 가정하면 안 된다.** 영숫자 코드가 76개 있다.

### Q8. 규칙으로 정할 수 있는 중요도

- **KIND `category`**(실측 분포): 수시공시 103, 지분공시 80, 신고사항 69, 정기공시 59, 공정공시 43, 파생상품시장공시 42, 특수공시 29, 시장조치 22, 발행공시 17, 공정위공시 7, ETN 7, 워런트 2.
- **DART `pblntf_ty`·`pblntf_detail_ty`**: A 정기, B 주요사항보고, C 발행, D 지분, E 기타, F 외부감사, G 펀드, H 자산유동화, I 거래소, J 공정위.
- **규칙 예** (보유 종목 기준):

| 등급 | 조건 (제목 또는 유형) | 동작 |
|---|---|---|
| 긴급 | 매매거래정지, 관리종목, 상장폐지, 불성실공시, 투자주의·경고·위험, 조회공시 요구, 감사의견 비적정, 횡령·배임, 회생·부도 | 즉시 알림 + AI 깨움 |
| 높음 | 유상증자, 전환사채·신주인수권부사채 발행, 감자, 합병·분할, 최대주주 변경, 자기주식 취득·처분·소각, 현금·현물 배당 결정, 영업(잠정)실적, 단일판매·공급계약, 주요사항보고서(B) | AI 깨움 |
| 보통 | 정기보고서(A), 대량보유·임원 소유상황(D), 기업설명회 개최, 주주총회 소집 | 브리핑에 포함 |
| 낮음 | 파생상품시장공시, 주식선물 가격제한폭 확대요건, ETN·워런트, 펀드(G), 자산유동화(H), 일괄신고 추가서류 | 수집만 |

- `[정정]`·`[기재정정]`은 원래 공시의 등급을 이어받되 한 단계 낮춘다. 단, 실적·계약 금액 정정은 같은 등급으로 둔다. 금액 비교는 본문이 필요하므로 미확인이다.
- 이 표는 제목 키워드에 기댄다. 하루치 KIND 표본(478건)에서는 상위 제목 30종이 모두 위 규칙의 어느 칸에 들어갔다. 장기 정확도는 미확인이다.

### Q9. 미국 지수·금리·원자재·달러 지수

| 지표 | 실시간에 가까운 값 | 공식 일별 값 | 확인 |
|---|---|---|---|
| S&P 500, 나스닥, 다우 | 토스 미국 시세 API로 SPY·QQQ·DIA(ETF) 현재가. 토스 스펙이 국내·미국 주식 시세를 제공함(`docs/toss-openapi-overview.md`) | FRED `SP500`, `NASDAQCOM`, `DJIA` (FRED API 키 필요. SP500 등 제3자 저작권 시리즈는 "own personal use" 밖이면 원소유자 허락이 필요) | 토스 ETF 대체는 스펙상 가능. 실제 호출은 R-01 범위. 이번 조사 환경에서 FRED 공개 CSV(`fredgraph.csv`)는 HTTP/2 INTERNAL_ERROR와 응답 없음으로 받지 못함(미확인) |
| 미국 금리 | 토스 IEF·TLT·SHY ETF | **미 재무부 일별 국채 금리 XML** `https://home.treasury.gov/resource-center/data-chart-center/interest-rates/pages/xml?data=daily_treasury_yield_curve&field_tdr_date_value_month=YYYYMM` (키 없음). FRED `DGS2`, `DGS10`, `DFF` | 재무부 XML 확인: 2026-11-10 값(10년 4.98%, 2년 4.73%) |
| 원자재 | 토스 USO·GLD ETF | FRED `DCOILWTICO`, `DCOILBRENTEU` | FRED 미확인. 금 현물 일별 공식 무료 시리즈는 못 찾음(미확인) |
| 달러 지수 | 토스 UUP ETF | ICE DXY는 상용 지수라 공식 무료 API를 못 찾음(미확인). 대신 FRED `DTWEXBGS`(연준 Broad 달러 지수)를 쓰되, DXY와 다른 지수임을 화면에 표시 | 미확인 |
| 변동성 | - | FRED `VIXCLS` | 미확인 |
| 한국 기준금리 | - | 한국은행 ECOS `722Y001` | sample 키로 확인 |
| 원/달러 | 토스 환율 API(참고용) | FRED `DEXKOUS`, ECOS | 미확인 |

- FRED API 약관: 키 등록이 필수이고 수치 한도는 없다. 단 "unreasonable amount of bandwidth"는 금지한다. 제3자 시리즈는 notes에 "Copyright"가 표시된다.
- 권고: 장중 값은 토스 ETF(이미 연결된 경로, 새 키 없음)로 받고, 일별 공식값은 재무부 XML(키 없음)과 FRED(키 1개)로 받는다. 화면에는 "ETF 기준 근사"인지 "공식 지수"인지 표시한다(ACC-3, ACC-4).

## 3. 선택지 비교표와 추천

### 3.1 소스별 요약표 (추가 산출물)

| 소스 | 주소 | 인증 | 한도 | 권장 주기 | 이용 조건 요약 | 위험 |
|---|---|---|---|---|---|---|
| KIND 오늘의 공시 RSS | `kind.krx.co.kr/disclosure/rsstodaydistribute.do?method=searchRssTodayDistribute&mktTpCd=0&currentPageSize=100` | 없음 | 미확인 | 국내 장중 2분, 그 외 30분. 앱 시작 시 500으로 따라잡기 | 약관 미확인 | 봇 관리(Akamai) 차단 가능. 종목코드 없음. 지난 날짜는 못 받음 |
| DART Open API | `opendart.fss.or.kr/api/list.json`, `corpCode.xml`, `fnlttSinglAcntAll.json` | 키 | 하루 약 2만 (020) | 목록 10분(장중), 고유번호 하루 1회, 재무 분기 1회 | 저장·가공 제한 조항 없음. 1인 1키 | 시각 없음. 3개월 기간 제한 |
| DART RSS | `dart.fss.or.kr/api/todayRSS.xml`, `companyRSS.xml?crpCd=` | 없음 | 미확인 | KIND 장애 시 대체 | 미확인 | 50건 한계, 지분 제외 |
| 한국경제 증권 RSS | `www.hankyung.com/feed/finance` | 없음 | 미확인 | 30분 (CDN 지연 약 55분이라 더 자주 받을 이유가 없음) | 기사에 AI 활용 금지 문구 | description 없음, 늦음 |
| 매일경제 증권 RSS | `www.mk.co.kr/rss/50200011/` | 없음 | 미확인 | 10분 | AI학습 이용 금지 문구, AI 봇 robots 차단 | 304 안 됨 |
| 연합뉴스 마켓+ RSS | `www.yna.co.kr/rss/market.xml` (+`economy.xml`) | 없음 | 미확인 | 10분 | 비상업·개인 용도만. AI 학습 및 활용 금지 | `read_article` 금지 권고 |
| 이데일리 증권 RSS | `rss.edaily.co.kr/stock_news.xml` | 없음 | 미확인 | 10분 | 무단전재 금지. RSS 조건 미확인 | 공식 목록 페이지 미확인 |
| 네이버 검색 API(HUB) | `naverapihub.apigw.ntruss.com/search/v1/news` | NCP 키 | 월 775,000(검색 통합), 50 RPS | 저장하지 않는 즉시 조회만 | 저장·캐시·가공 금지. 가공 없이 노출 | 유료 전환 예정. 신규 가입 조건 미확인 |
| Google 뉴스 RSS | `news.google.com/rss/search?q=…&hl=ko&gl=KR&ceid=KR:ko` | 없음 | 미확인 | 보유 종목 60분 | 개인 피드 리더 표시 목적만 | 링크가 중계 주소. "그 밖의 이용 금지" |
| Yahoo 헤드라인 RSS | `feeds.finance.yahoo.com/rss/2.0/headline?s=…` | 없음 | 미확인 | 미국 보유 종목 30~60분 | 수정 없이 표시하고 출처 링크. 자동 수집 금지 조항 | 비공식. UA에 따라 404 |
| SEC EDGAR | `data.sec.gov/submissions/CIK….json`, `browse-edgar?action=getcurrent&output=atom` | 없음(연락처 UA 필요) | 10 req/s | 미국 보유 종목 15분(미국 장중) | 공공 자료 | 사용자 연락처 결정 필요 |
| 한국은행 RSS | `www.bok.or.kr/portal/bbs/B0000552/news.rss?menuNo=200690` | 없음 | 미확인 | 60분 | 미확인 | 응답이 크다(225~465KB), 304 안 됨 |
| 연준 RSS | `www.federalreserve.gov/feeds/press_all.xml` | 없음 | 미확인 | 60분 (FOMC 당일 10분) | 미확인 | 낮음 |
| 미 재무부 금리 XML | `home.treasury.gov/…/xml?data=daily_treasury_yield_curve&field_tdr_date_value_month=YYYYMM` | 없음 | 미확인 | 하루 1회(미국 장 마감 후) | 미확인 | 낮음 |
| FRED API | `api.stlouisfed.org/fred/series/observations` | 키 | 수치 없음 | 하루 1회 | 제3자 시리즈는 개인 이용 한정 | 키 하나 더 필요 |

### 3.2 종목별 국내 뉴스: 네이버 대 Google 대 RSS 매칭만

| 기준 | 네이버 검색 API (HUB) | Google 뉴스 RSS | 언론사 RSS 4개 + 로컬 매칭 |
|---|---|---|---|
| 신규 사용 가능 | HUB 가입 필요(개발자센터 신규 중단) | 키 없음 | 키 없음 |
| 저장 | **금지**(24시간 캐시 수준) | 피드 조건이 "개인 피드 리더 표시"뿐 | 개인·비상업이면 법상 여지 큼 |
| 원문 링크 | `originallink` 있음 | 중계 주소 | 원문 |
| 범위 | 국내 언론 전체 | 넓음 | 4개 언론사만 |
| 비용 | 한시 무료 후 유료 | 무료 | 무료 |

**추천: 언론사 RSS + 로컬 매칭을 기본으로 한다.** 공식 피드라 조건이 가장 단순하고, 원문 링크라 `read_article` 허용 목록과 맞는다. 보유 종목의 빈틈은 Google 뉴스 RSS로 메운다. 이때 제목과 링크만 표시하고 AI 입력에는 제목만 넣으며, 항목은 7일 뒤 지운다. 네이버는 "검색 결과를 즉시 보여 주기만 하는" 채팅 도구(저장하지 않음)로 선택 지원하거나 아예 뺀다.

- 근거: 네이버는 설계의 핵심 동작(저장, 묶음, 매칭)이 약관과 정면으로 부딪친다. Google은 조건이 좁지만 개인 피드 리더 용도와 가깝다.
- **추천이 틀렸다고 볼 조건**:
  1. 4개 언론사 RSS로 보유 종목 뉴스 재현율이 너무 낮을 때(예: 보유 종목 주간 기사 중 Google 뉴스에만 있는 비율이 50%를 넘음, 측정 미실시).
  2. Google이 RSS를 차단하거나 중계 링크를 풀 수 없게 될 때.
  3. 네이버 HUB 약관에 저장 허용 특약이 생길 때.

### 3.3 국내 공시: KIND 대 DART

**추천: KIND RSS를 주 소스(2분)로, DART `list.json`을 보강 소스(10분)로 쓴다.**
- KIND는 분 단위 시각, 지분공시 포함, 하루 전체를 한 번에 주고 키가 없다.
- DART는 `corp_code`·`stock_code`·정정 표시(`rm`)를 준다. 이것으로 재무 API와 연결한다.
- 두 소스는 `(접수일, 번호 10~14자리, 시장)`으로 맞춘다. 근거는 2.Q2 실측이다.
- 틀렸다고 볼 조건: KIND가 봇 관리로 정기 호출을 막을 때(403·챌린지 응답). 그때는 DART RSS 2분과 DART 목록 10분으로 바꾼다.

## 4. 위험과 미확인 항목

**위험**
- **네이버**: 개발자센터 키가 이미 있어도 2027-06-30에 끝나고 HUB로 옮겨야 한다. HUB는 유료로 바뀔 예정이다. 저장 금지가 설계와 충돌한다.
- **KIND 차단 가능성**: Akamai Bot Manager 쿠키(`bm_s`)가 발급됐다. 2분 주기로 장기 호출할 때 막힐지는 미확인이다.
- **AI 활용 금지 문구**: 연합·한경·매경이 기사에 AI 학습·활용 금지를 단다. `read_article`로 본문을 AI CLI(외부 서버)에 보내는 것이 이와 충돌할 수 있다. 연합뉴스는 "활용"까지 막는다.
- **Google 뉴스**: "그 밖의 이용은 명시적으로 금지". 앱이 AI 분석에 쓰는 것이 "개인 피드 리더 표시"를 넘는다고 볼 여지가 있다.
- **Yahoo**: 문서가 없는 엔드포인트라 예고 없이 바뀔 수 있다. 약관은 자동 수집을 금지한다.
- **한경 피드 지연** 약 55분(CDN 캐시). 장중 감시용으로 부적합하다.
- **종목 코드 형식**: 영숫자 코드(76개)가 이미 있다. 6자리 숫자를 가정한 코드는 깨진다.
- **단일 연결 군집의 사슬 효과**: 실측으로 확인했다.

**미확인**
- KIND RSS 이용약관과 호출 한도.
- DART 분당 한도, `corpCode.xml` 갱신 주기, 정정본과 원본을 잇는 API 필드.
- DART RSS와 KIND RSS의 조건부 GET 지원.
- KIND `acptNo`와 DART `rcpNo` 대응 규칙의 공식 근거(코넥스 등 다른 시장의 9번째 자리 값 포함).
- 한경·이데일리 RSS 이용 조건 원문. 이데일리 공식 증권 피드 목록.
- 언론사 RSS 호출 한도와 차단 기준.
- NAVER API HUB 가입 조건(결제 수단 등록 여부), 유료 단가, 저장 허용 특약 유무.
- Google 뉴스 RSS의 호출 한도와 중계 링크를 원문 링크로 푸는 공식 방법.
- SEC EDGAR 실제 응답(호출하지 않음). 최신 공시 Atom 피드의 갱신 주기.
- FRED 공개 CSV가 이 환경에서 응답하지 않은 원인. FRED 각 시리즈의 지연. 금 현물 공식 무료 시리즈.
- BLS 일정 접근 방법(일반 UA로 403).
- 미국 실적 일정의 공식 무료 소스. 국내 미래 실적 발표일 달력.
- 증권사 리포트 요약의 공식 소스.
- KRX OPEN API의 한도와 지연.
- 임베딩 모델(e5-small 등)의 한국어 제목 묶음 정확도와 메모리 사용량.
- 제목의 저작물성을 다룬 판례 원문. 비학습 AI 분석의 저작권법상 위치. 기사 하단 금지 문구의 계약상 구속력.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서 | 항목 | 바꿀 내용 |
|---|---|---|
| design.md 5.1 표 | 네이버 검색 API 행 | "공식 API (무료 키, 하루 2.5만 건)"를 "개발자센터 신규 중단(2026-07-30), NAVER API HUB(월 77.5만, 한시 무료). **결과 저장 금지**"로 바꾼다. 수집 소스에서 빼거나 "저장하지 않는 즉시 조회 도구"로 바꾼다 |
| design.md 5.1 표 | Google 뉴스 행 | "네이버 키가 없을 때 대체"를 "보유 종목 보조. 피드 조건이 개인 피드 리더 표시 목적. 링크가 중계 주소"로 바꾼다 |
| design.md 5.1 표 | DART·KIND 행 | DART 목록 API에는 시각이 없음을 적는다. KIND를 저지연 주 소스로 하고 DART RSS(키 없음)를 대체 소스로 추가한다. 응답 확인 날짜(2026-11-11)를 적는다 |
| design.md 5.1 표 | SEC EDGAR 행 | 요구 사항을 확정해 적는다: 연락처가 든 UA, 10 req/s, 키 없음. 엔드포인트도 적는다 |
| design.md 5.1 본문 | `read_article` | 소스별 `allow_ai_read`(기본 끔, 연합뉴스는 켤 수 없음 권고)를 추가하고 AI 활용 금지 문구가 있음을 적는다 |
| design.md 5.1 본문 | 종목 매칭 | 경계·조사 규칙, 일반 명사 종목명 목록, 그룹명 접두 규칙, 약칭 표(사용자 편집)를 추가한다. 영숫자 종목 코드를 명시한다 |
| design.md 5.1 본문 | 중복 제거 | "제목 유사도"를 "리더 기준 2-gram Jaccard ≥0.45, 같은 종목, 48시간 창. 단일 연결 금지. 경계 구간만 임베딩"으로 구체화한다 |
| design.md 5.2 표 | 주기 | DART·KIND 2분은 KIND로 한다. DART 목록은 10분으로 바꾼다. 한경 RSS는 30분으로 늘린다(CDN 지연). "하루 약 300건" 계산을 고친다 |
| design.md 5.2 본문 | ETag | "RSS는 ETag와 If-Modified-Since로…"를 소스별로 지원 여부가 다르다고 고친다(304 확인: 이데일리, 한경, 연준. 안 됨: 매경, 연합, Google, KIND, 한은) |
| design.md 7.5 | API-5 | 네이버 한도 단위가 월(775,000)로 바뀐 점을 반영하거나 항목에서 네이버를 뺀다. DART만 남긴다 |
| design.md 9장 | 위험 표 | "뉴스 저작권" 행에 AI 활용 금지 문구, 네이버 저장 금지, Google 피드 조건을 추가한다. "미결정: SEC EDGAR"에 문서로 확인한 요구 사항을 적는다 |
| design.md 3장 | `get_market_overview` | 미국 지수·금리·원자재·달러를 토스 ETF 근사 + 재무부 XML + FRED(키 1개 추가)로 채운다고 적는다. 8장 설정 "뉴스·데이터" 탭에 FRED 키를 추가한다 |
| architecture.md | `NewsSource` 포트 | 소스 메타데이터를 추가한다: `retention`(보관 기간, 예 네이버 24시간), `allow_ai_read`, `supports_conditional_get`, `requires_user_agent` |
| architecture.md | 도메인 타입 | 종목 코드를 6자리 숫자가 아닌 영숫자 문자열로 둔다. 공시 키를 `(source, receipt_no)`로 두고 KIND와 DART 대응 규칙을 따로 둔다 |
| tasks.md | N-06 | "네이버 검색 API, Google 뉴스 RSS 대체"를 "Google 뉴스 RSS 보조 + (선택) 네이버 HUB 즉시 조회, 저장 금지 테스트"로 바꾼다 |
| tasks.md | N-04 | 종목 마스터에 KIND 상장법인목록(키 없음)과 DART `corpCode.xml`을 함께 쓴다 |
| tasks.md | N-05 | KIND RSS에 "회사명→코드 대응 실패 목록", "acptNo와 rcpNo 대응" 테스트를 추가한다 |
| tasks.md | N-07 | Yahoo는 브라우저형 UA 필요, SEC는 사용자 연락처 설정이 필요하다고 적는다 |
| tasks.md | 신규 | 미국 시장 지표 소스(재무부 XML, FRED) 작업을 추가한다 |
| research.md | R-13 | 이 문서 Q5(AI 활용 금지 문구, 네이버 저장 금지)를 R-13 질문에 넣는다 |

## 6. 출처 목록

모두 2026-11-11에 확인했다.

**공시**
- OpenDART 공시검색 API 개발가이드: https://opendart.fss.or.kr/guide/detail.do?apiGrpCd=DS001&apiId=2019001
- OpenDART 고유번호 API: https://opendart.fss.or.kr/guide/detail.do?apiGrpCd=DS001&apiId=2019018
- OpenDART 단일회사 전체 재무제표 API: https://opendart.fss.or.kr/guide/detail.do?apiGrpCd=DS003&apiId=2019020
- OpenDART 이용약관(2020-01-21 시행): https://opendart.fss.or.kr/intro/terms.do
- DART RSS 서비스 소개: https://dart.fss.or.kr/introduction/content6.do
- DART RSS 실측: https://dart.fss.or.kr/api/todayRSS.xml, https://dart.fss.or.kr/api/companyRSS.xml?crpCd=00126380
- KIND 오늘의 공시(RSS 주소 생성 스크립트 포함): https://kind.krx.co.kr/disclosure/todaydisclosure.do?method=searchTodayDisclosureMain
- KIND RSS 실측: https://kind.krx.co.kr/disclosure/rsstodaydistribute.do?method=searchRssTodayDistribute&repIsuSrtCd=&mktTpCd=0&searchCorpName=&currentPageSize=500
- KIND 상장법인목록 내려받기 실측: https://kind.krx.co.kr/corpgeneral/corpList.do?method=download&searchType=13
- SEC Accessing EDGAR Data (Last Reviewed 2024-06-26): https://www.sec.gov/search-filings/edgar-search-assistance/accessing-edgar-data
- SEC EDGAR APIs (Last Reviewed 2025-04-08): https://www.sec.gov/search-filings/edgar-application-programming-interfaces
- SEC RSS Feeds (Last Reviewed 2026-06-24): https://www.sec.gov/about/rss-feeds
- SEC Webmaster FAQ, company tickers: https://www.sec.gov/about/webmaster-frequently-asked-questions, https://www.sec.gov/file/company-tickers

**뉴스**
- 네이버 뉴스 검색 API 문서: https://developers.naver.com/docs/serviceapi/search/news/news.md
- 네이버 오픈API 이용약관(2026-10-07 개정, 부칙의 HUB 이관 일정): https://developers.naver.com/products/terms/
- NAVER API HUB 개요(2026-09-17 갱신): https://guide.ncloud-docs.com/docs/apihub-overview
- NAVER API HUB 뉴스 검색 API(2026-09-17 갱신): https://api.ncloud-docs.com/docs/naver-api-hub-search-news
- NAVER API HUB 서비스 이용약관(2026-06-25 시행, PDF): https://xv-ncloud.pstatic.net/images/provision/NAVER_API_%C3%AC%C2%84%C2%9C%C3%AB%C2%B9%C2%84%C3%AC%C2%8A%C2%A4_%C3%AC%C2%9D%C2%B4%C3%AC%C2%9A%C2%A9%C3%AC%C2%95%C2%BD%C3%AA%C2%B4%C2%80_APIHUB_%C3%AC%C2%B5%C2%9C%C3%AC%C2%A2%C2%85_260624_KR_clean_1782291589723.pdf
- 보조(2차): 토큰포스트 HUB 이관 기사 https://www.tokenpost.kr/news/ai/403521
- 연합뉴스 RSS 안내: https://www.yna.co.kr/rss/index
- 매일경제 RSS 안내: https://www.mk.co.kr/rss
- 한국경제 피드 목록: https://www.hankyung.com/feed
- 각 언론사 robots.txt: https://www.mk.co.kr/robots.txt, https://www.edaily.co.kr/robots.txt, https://www.yna.co.kr/robots.txt, https://www.hankyung.com/robots.txt
- 기사 하단 문구 실측: https://www.hankyung.com/article/202611110253i, https://www.yna.co.kr/view/AKR20261111157200030, https://www.mk.co.kr/news/stock/12205485, http://www.edaily.co.kr/news/newspath.asp?newsid=05362806645644736
- Google 뉴스 RSS(피드 `<copyright>` 문구): https://news.google.com/rss/search?q=%EC%82%BC%EC%84%B1%EC%A0%84%EC%9E%90&hl=ko&gl=KR&ceid=KR:ko
- Google News Terms of Use(2006, 구버전 가능성): https://www.google.com/intl/en_us/terms_google_news.html
- Yahoo Terms of Service(RSS 조항, 자동 수집 조항): https://legal.yahoo.com/xw/en/yahoo/terms/otos/index.html

**거시·시장 지표**
- 한국은행 RSS 목록: https://www.bok.or.kr/static/view/popup/rss_popup.html
- 한국은행 ECOS API(sample 키 응답): https://ecos.bok.or.kr/api/StatisticSearch/sample/json/kr/1/5/722Y001/D/20261101/20261111/0101000
- 연준 RSS: https://www.federalreserve.gov/feeds/press_all.xml, https://www.federalreserve.gov/feeds/press_monetary.xml, https://www.federalreserve.gov/feeds/speeches.xml
- 연준 FOMC 일정: https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm
- BEA RSS: https://apps.bea.gov/rss/rss.xml
- BLS(403 확인): https://www.bls.gov/feed/bls_latest.rss
- 미 재무부 일별 국채 금리 XML: https://home.treasury.gov/resource-center/data-chart-center/interest-rates/pages/xml?data=daily_treasury_yield_curve&field_tdr_date_value_month=202611
- FRED API 문서와 약관: https://fred.stlouisfed.org/docs/api/fred/, https://fred.stlouisfed.org/docs/api/terms_of_use.html
- KRX OPEN API: https://openapi.krx.co.kr/

**법령·판례**
- 저작권법([시행 2026. 10. 29.] 법률 제21595호) 원문: https://www.law.go.kr/LSW/lsInfoP.do?lsiSeq=285693
- 대법원 2006. 9. 14. 선고 2004도5350 판결(요지, 2차): https://www.nepla.ai/case/대법원/2004도5350
- 대법원 2007다354(요지, 2차): https://www.nepla.ai/case/%EB%8C%80%EB%B2%95%EC%9B%90/2007%EB%8B%A4354

**라이브러리**
- crates.io API: aho-corasick 1.1.5, lindera 6.2.0(`embed-ko-dic`), fastembed 7.7.0, ort 2.0.0-rc.15, feed-rs 3.0.0, simhash 0.3.0, gaoya 0.2.2: https://crates.io/api/v1/crates/{이름}
- fastembed-rs README(지원 모델): https://github.com/Anush008/fastembed-rs

## 부록. 실행한 실험

스크립트는 모두 `/private/tmp/claude-501/-Users-choegihwan--t3-worktrees-sure-t3-7f34ca39-5542c437df4f301a/d672a99d-9dd8-4133-b4ab-9b52494be3de/scratchpad/research/R-07/`에 있다.

| 스크립트 | 내용 | 결과 |
|---|---|---|
| `feeds.txt` + curl 반복 | RSS 9개 상태·형식·크기 | 전부 200 (2.Q1 표) |
| `inspect.py` | 항목 수, 필드, 첫·끝 시각 | 2.Q1 표 |
| 조건부 GET | `If-None-Match`/`If-Modified-Since` 재요청 | edaily·fed·hankyung 304, yna 200 |
| `match.py` | KIND 480건과 DART RSS 50건 번호 대조 | 9번째 자리 규칙 23쌍, 직접 일치 13건은 모두 다른 공시 |
| `exp_match.py`, `exp_match2.py` | 2,767개 종목명으로 제목 390개 매칭 | 127개 → 109개 제목, 오탐 유형은 2.Q7 |
| `exp_cluster.py`, `exp_cluster2.py` | 2-gram Jaccard 단일 연결과 리더 군집 비교 | 0.17초. 단일 연결은 회사가 섞임, 리더 방식은 섞이지 않음 |
