# N 묶음 설계 결정 기록

문서가 정하지 않은 세부 선택지를 가장 단순하고 안전한 쪽으로 확정해 기록한다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| N-01 | `domain::DataSource` 17개 닫힌 열거형(`HttpDataSource` 16개 + `WebSearch`)과 호스트 기준 14개 `RateLimitBucket`(`Kind`, `OpenDart` 공유)으로 동시 1건·최소 5초 간격 강제 | 논리 소스 16개마다 개별 버킷 사용 |
| N-01 | 이데일리 RSS도 `https` 전용 유지(리다이렉트 금지, `Policy::none()`, 3xx 차단) | 이데일리 단일 호스트만 `http` 예외 허용 |
| N-01 | `Retry-After`는 정수 초만 해석해 버킷 다음 허용 시각 연장 | IMF-fixdate HTTP-date 문자열까지 해석 |
| N-01 | SEC EDGAR 연락처는 식별자 + 유효 이메일 1개 + 가시 ASCII 256바이트 검증, 미설정 시 연결 0건 | 비어 있지 않은 문자열만 확인하고 형식 오류를 런타임 403에 위임 |
| N-01 | `SourceStatusRepository` 포트·유스케이스·`SourceStatusChanged` 이벤트 계약만 확정하고 영속 저장·UI는 후속(N-14/S)에 인계 | N-01에서 SQLite 테이블·고급 상태 UI까지 확장 |
| N-19 | 접미사 없는 단독 4자리 연도(`"2026"`)와 6자리 국내 종목 코드(`"005930"`)는 거부, 연도·분기 명시 표기(`2026년`, `Q3 2026`, `FY2026`)만 최대 2건 허용 | `2000..=2099` 단독 4자리나 종목 코드 허용 목록 통과 |
| N-19 | 연도·분기 마스킹 후 남은 ASCII 숫자가 총 2개 이상이면 문자 삽입 분할 포함 전부 거부 | 연속 2자리·구분자 분할만 거부하고 영문자 내부 단일 숫자 허용 |
| N-19 | 감사 로그(`AuditKind::WebSearchChecked`)에 검색어 원문을 남기지 않고 SHA-256 다이제스트·모드·닫힌 판정/사유만 기록(감사 쓰기 실패 시 fail-closed) | 통과된 코드 검색어에 한해 원문 평문 저장 |
| N-19 | 등록 실명(2자 이상)은 종목명과 겹쳐도 거부, 1자 성씨는 등록 거부, `"사조"` 등 수사 결합 상호도 보수적으로 거부 | 종목 마스터 예외 표를 두어 실명·수사 충돌 일부 통과 |
| N-02 | 깨진 RSS 피드(비UTF-8·HTML 오류 페이지·미종결 XML·전체 항목 URL 무효)는 `InvalidResponse(Schema)`로 거부하고, 정상 피드 내 개별 손상 항목만 건너뜀 | 개별 손상 항목 하나에도 전체 피드 폐기 |
| N-17 | `SourceAiBodyReadSettings`에서 `Yonhap`·`GoogleNews`는 켜기 자체를 타입·오류로 봉인하고, `ArticleReader`는 본문 읽기가 켜진 언론사 RSS의 `https` 기사 도메인(`www.edaily.co.kr`, `finance.yahoo.com` 포함)만 일회성 읽기 허용 | 웹 출처나 `http` 기사 주소까지 일회성 읽기 허용 |
| N-03 | DART `list.json`의 정정 공시는 `[기재정정]`·`[첨부정정]`·`[첨부추가]`·`[변경등록]`·`[발행조건확정]`·`[정정]` 접두와 `rm`의 `정`/`철` 표시를 분해해 `(corp_code, 기본 보고서명)`으로 원공시와 묶고, 접수 시각이 없으므로 `PublicationTime::NotProvided`로 기록 | 접수일의 00:00을 임의 발행 시각으로 채우거나 정정 접두를 원문 그대로 별도 공시로 분리 |
| N-04 | `corpCode.xml` ZIP 해제는 새 외부 패키지 없이 중앙 디렉터리+`flate2` Deflate(압축 해제 상한 64MiB)로 구현하고, 국내 마스터 대조에서 미매칭(`MissingInDart`)과 중복 종목코드(`AmbiguousStockCode`)를 모두 대응 실패 목록으로 반환 | 외부 `zip` 크레이트 추가 또는 중복 종목코드 중 첫 항목을 임의 선택 |
| N-05 | KIND 회사명→코드 대응은 법인격 표기(`(주)`·`주식회사` 등)·공백 정규화 후 국내 마스터(T-09)와 선택 DART 대응표로 매칭하고, KIND 차단(403·429·HTML 챌린지) 시 DART RSS+선택 DART 목록으로 자동 전환(중복 시 시각 있는 DART RSS 우선). 실측 S-20은 테스트 중 외부 네트워크 호출 금지 규칙에 따라 실서버 장기 호출 보류·가짜 서버 차단 전환으로 검증 | 실서버 반복 호출로 Akamai 차단 시점 실측 또는 KIND 실패 시 오류로 중단 |
| N-06 | Google 뉴스 검색 RSS 질의는 N-19 `ApprovedWebSearchQuery`만 받고, RSS `<description>`을 수집 시점에 폐기해 표시 모델(`GoogleNewsDisplayItem`)은 제목·링크·언론사만, AI 입력은 제목만(`AiReadPolicy::TitleOnly`) 전달하며 정확히 7일(604,800초) 경과 항목 삭제 함수를 제공 | 자유 문자열 검색 허용 또는 RSS 설명문을 저장 후 UI/AI 단계에서 필터링 |
| N-07 | Yahoo 헤드라인 RSS는 검증된 심볼 최대 50개를 묶어 브라우저형 `User-Agent`로 조회하고, SEC EDGAR는 연락처 미설정 시 `SourceHttpError::MissingContactUserAgent`를 `DataAvailability::NotConfigured`로 변환(네트워크 연결 0건)하며 `company_tickers.json`·`submissions/CIK.json`·현재 Atom 피드 모두 공통 5초 간격(초당 0.2회 ≤ 10회) 버킷을 공유 | SEC 연락처 미설정 시 오류 반환 또는 피드별 별도 버킷으로 초당 10회 근접 호출 |
| N-08 | `DartFundamentalsClient`는 `period_end` 월(3·6·9·12월)을 DART 보고서 코드(`11013`·`11012`·`11014`·`11011`)로 매핑하고 연결(`CFS`) 조회가 `"013"`(자료 없음)이면 별도(`OFS`)로 자동 대체하며, `KRW`·`USD` 외 통화 행은 오표기를 막기 위해 건너뜀 | 별도(`OFS`) 자동 대체 없이 실패 또는 미지원 통화를 `KRW`로 강제 변환 |
