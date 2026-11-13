# R-01 토스증권 Open API 실사용 조건

- 조사일: 2026-11-11 (모든 출처 확인일 같음)
- 기준 스펙: REST `info.version` 1.2.28 (OpenAPI 3.1.0), 웹소켓 AsyncAPI 3.0.0 `info.version` 1.2.3
- 저장소의 `docs/toss-openapi-1.2.28.json`은 조사일에 받은 `latest/openapi.json`과 바이트 단위로 같다 (sha256 `c5918961335ccd6b091577e307dc06ed505b58f83e4d99f0c4617de14ceb036a`). 저장소의 `docs/toss-openapi-overview.md`도 공식 `overview.md`와 같다.
- 실제 계좌·키로 하는 호출은 하지 않았다. 실험은 공개 문서 내려받기, 스펙 기계 분석, 인증 없는 GET 1회뿐이다.

## 1. 요약

1. 약관과 이용 안내는 AI·LLM 연동과 자동 주문을 막지 않는다. 오히려 토스가 직접 홍보한다. 대신 주문 오류·중복·손실과 키 유출 책임은 전부 사용자에게 있다. 시세는 본인 매매 목적으로만 쓰고 제3자에게 줄 수 없다. 짧은 시간의 대량 주문과 비정상 조회 부하에는 매매 제한, 1개월 이용 중지, 해지가 걸려 있다.
2. 키에 권한 범위가 없다 (`scopes: {}`). 읽기 전용 키도, 주문 제외 키도 없다. 서버 쪽에서 주문만 막는 수단은 찾지 못했다. 주문 차단은 앱 구조로 해야 한다.
3. 토큰은 클라이언트당 1개다. 새로 발급하면 이전 토큰이 즉시 폐기되고, 그 토큰으로 열린 웹소켓도 끊긴다. 응답에 `clientOrderId`가 없고 거래내역에 고유 ID가 없다. 그래서 응답 불명 주문 확인과 거래내역 중복 제거는 내용 대조로 해야 한다.
4. 현금 잔고(예수금) API는 없다. 통화별 `cashBuyingPower`가 가장 가까운 값이다. 1억 원 판정 환율, 허용 IP 개수·대역 등록, 모의 환경 여부는 공식 문서에 없어 미확인이다.
5. 스펙은 2026-06-01 이후 5개월 동안 22번 이상 바뀌었고, 같은 버전 번호인 채 내용이 바뀐 적이 있다. 거래내역 API도 그렇게 추가됐다. 버전 번호 대신 내용 해시로 변경을 감지해야 한다.

## 2. 질문별 답

### Q1. 이용약관·이용 가이드의 금지·제한 사항

**약관** (「오픈 API 서비스 이용 약관」, 제정 2026-05-18, 개정·시행 2026-08-12) [S1]

| 조항 | 내용 | 이 앱과의 관계 |
|---|---|---|
| 제5조 ② | 보안코드(앱키, 시크릿키)를 제3자에게 대여·위임·양도·누설 금지 | 키가 AI CLI 프로세스나 프롬프트에 닿으면 안 된다 (SEC-1, SEC-2와 일치) |
| 제5조 ③ | 시세(국내·해외주식) 정보는 "개인투자자 본인의 매매 목적에 한하여" 이용하고, 제3자에게 제공·배포·상업적 활용 금지 | 배포하지 않는 개인 앱이면 목적은 맞다. 시세를 LLM 제공자에게 보내는 것이 "제3자 제공"인지는 약관에 정의가 없다 → R-13에서 판단 |
| 제5조 ④ | 법인 고객 제외 | 해당 없음 |
| 제7조 | 만 19세 미만 승인 거부 | 해당 없음 |
| 제8조 ① | "비정상적인 데이터 조회에 의한 부하"가 생기면 1개월 이용 중지 | 스케줄러·속도 제한기가 보수적이어야 한다 |
| 제9조 ① | 비정상 조회 부하가 3회 이상이면 해지 | 같음 |
| 제11조 | 호출 횟수 제한 정책은 회사가 게시. 트래픽 급증 시 "별도 공지 또는 게시 없이" 제한·하향 가능 | 한도를 상수로 박지 말고 응답 머리말을 따른다 (T-03와 일치) |
| 제12조 | 점검 시간에는 잔고 조회 등이 실제와 다를 수 있음 | 점검 중 스냅샷은 신뢰도를 낮춰 표시 |
| 제14조 | 약관 변경은 시행 1개월 전 게시. 불리한 변경은 개별 통지 | |
| 제15조 | 서비스 내용 변경은 홈페이지 또는 API 홈페이지에 사전 게시 | Q9 참고. 실제로는 변경 공지 페이지를 찾지 못했다 |
| 제16조 ③ | 고객이 접근매체를 누설·방치하면 회사 면책 | |

**API 이용 안내** (Open API 소개 페이지의 "API 이용 안내" 펼침 영역) [S2]

- "API로는 입출금과 환전을 이용할 수 없어요."
- "API는 본인의 매매 목적으로만 이용할 수 있어요. 외부 배포나 상업적 용도로는 사용할 수 없으며"
- "자동화된 반복 매매, 알고리즘 매매 등으로 인한 주문 오류, 의도치 않은 체결, 중복 주문, 손실 등은 투자자 본인이 부담해야 해요." → 자동 주문은 금지가 아니라 책임 전가다.
- "API 호출 한도, 제공 기능 등은 … 사전 고지 없이 변경될 수 있어요."
- AI 연동: "외부 AI 도구의 분석 결과나 동작 오류로 발생한 손실에 대한 책임은 토스증권이 부담하지 않아요." "계좌 정보 및 거래 내역이 외부로 전송될 수 있어요. 외부 도구 사용으로 인한 정보 유출이나 오용에 대한 책임은 투자자 본인에게 있어요."

**AI·LLM 연동**: 금지 조항은 없다. 토스는 정식 출시 보도자료(2026-08-26)에서 "오픈API 키(key)를 챗GPT, 클로드 등 외부 대형언어모델에 연결"하는 것을 차별점으로 내세웠다 [S10].

**호출량**: 그룹별 TPS 표는 overview의 Rate Limits에 있다 [S4]. 더해서 FAQ에 "짧은 시간에 대량으로 주문을 반복하면(예: 소액 주문을 수십 분 내 수백 건) 어뷰징으로 판단되어 일정 시간 매매가 제한될 수 있습니다. 반복되면 제한 기간이 연장됩니다." [S3], overview에 "비정상 사용이 의심되는 일부 IP 대역에서는 요청 한도를 초당 최대 1회로 제한" [S4]이 있다.

**데이터 저장·가공**: 저장·가공을 금지하는 조항은 없다. 제한은 제5조 ③(목적 제한, 제3자 제공 금지)과 FAQ의 "데이터 이용 정책"뿐이다: "상업적 이용이 제외될 뿐만 아니라 비상업적인 용도라 하더라도 제 3자에게 배포하는 행위는 엄격히 금지" [S3]. 로컬 SQLite 저장은 이 조항에 걸리지 않는다고 읽힌다. 확정 판단은 R-13 몫이다.

### Q2. 키 권한 범위와 주문 차단 수단

- 스펙의 보안 스킴은 `oauth2ClientCredentials`이고 `scopes: {}`다. 토큰 요청 본문에는 `grant_type`, `client_id`, `client_secret`만 있고 `scope` 파라미터가 없다 [S5].
- 403 `forbidden`("요청에 필요한 권한이 부족합니다")은 공통 오류로 정의돼 있지만 어떤 권한을 말하는지 문서에 없다 [S5].
- 공식 문서·FAQ·약관·이용 안내 어디에도 읽기 전용, 주문 제외, 계좌별 권한 설정이 없다 [S1–S5]. 2차 자료도 마찬가지다 [S12, S13].
- 결론: **권한 범위를 정할 수 없다.** 키 하나로 조회와 주문이 모두 된다.

서버 쪽에서 쓸 수 있는 다른 수단과 한계:

| 수단 | 효과 | 한계 |
|---|---|---|
| 허용 IP 목록에서 IP 제거 | 모든 호출이 403. 열린 웹소켓도 `auth-revoked`로 끊김 [S6] | 조회까지 전부 막힌다. 주문만 막지 못한다 |
| 키 재발급·삭제 (WTS 설정) | 이전 키·토큰 무효 | 같음 |
| 계좌가 주문 불가 상태 (`account-restricted`) | 주문 거부 | 사용자가 켤 수 있는 설정인지 미확인 |
| overview 오류표의 `order-limit-exceeded`("주문 설정 한도를 초과했습니다") | 사용자 설정 주문 한도가 있음을 암시 | 어디서 설정하는지, Open API 주문에 적용되는지 미확인. OpenAPI JSON의 주문 오류 예시에는 이 코드가 없다 |

그래서 주문 차단은 앱 구조로 한다: 키는 코어만 들고, AI 프로세스는 키·네트워크·주문 코드에 닿지 않는다 (architecture.md 4장, SEC-1·2·5). 추가로 `broker-toss`의 HTTP 클라이언트를 읽기 전용과 주문용 두 타입으로 나눠, 주문 경로(POST `/api/v1/orders*`, `/conditional-orders*`, DELETE)를 `order-execution`만 만들 수 있는 타입에 두는 방법이 있다. 이는 R-09의 타입 상태 설계와 같은 원리다.

### Q3. 허용 IP

| 항목 | 답 | 근거 |
|---|---|---|
| 등록 위치 | WTS 설정 > Open API > 허용 IP 관리 | [S4] |
| 미등록 IP | REST는 403 `ip-not-allowed`, 토큰 발급은 OAuth2 `access_denied`, 웹소켓 핸드셰이크는 403 | [S4, S5, S6] |
| 등록 개수 | **미확인**. 공식 문서와 2차 자료 모두 수치가 없다 | |
| 대역(CIDR) 등록 | **미확인**. 공식 문서에 없다. 참고로 비공식 CLI는 허용 목록을 단일 주소로만 다룬다 (`netip` 주소 정규화) [S11]. 이것은 근거가 아니라 정황이다 | |
| 변경 반영 | 즉시 반영 시간은 **미확인**. 다만 웹소켓은 "허용 IP 목록에서 접속 IP 가 빠지거나 … `auth-revoked` 에러 프레임을 보낸 뒤 연결을 닫습니다"라고 하므로, 서버가 연결 중에도 목록을 다시 확인한다 [S6] | |
| 클라우드 대역 | "클라우드·데이터센터 대역이라는 이유로 별도 차단하지는 않습니다." | [S3] |
| 유동 IP 권장 | 공식 답은 하나다: "실행할 때마다 공인 IP 가 바뀌는 환경 … 이면 … 차단됩니다. 고정 IP 를 확보한 뒤 허용 IP 목록에 등록하세요." | [S3] |

가정용 유동 IP 사용자의 선택지:

| 방법 | 장점 | 단점 |
|---|---|---|
| (가) 현재 IP를 등록하고, `ip-not-allowed`가 나면 안내 화면을 띄움 (REL-1) | 추가 비용·구성 없음. 가정용 회선의 공인 IP는 공유기를 재부팅하지 않으면 오래 유지되는 경우가 많다 (이 빈도는 이 조사에서 확인하지 않음) | 바뀔 때마다 사람이 WTS에 들어가 바꿔야 한다. 그동안 동기화와 주문이 멈춘다 |
| (나) 고정 IP가 있는 출구(통신사 고정 IP 상품, 고정 IP VPN, 개인 VPS를 WireGuard 출구로 사용) | IP가 바뀌지 않는다. TLS는 토스까지 그대로라 출구 장비가 키를 보지 못한다 | 비용, 출구 장비가 죽으면 전부 멈춤, 출구 장비 관리 책임 |
| (다) 비공식 CLI처럼 WTS 내부 API로 허용 IP를 자동 교체 | 자동화 | 공식 API가 아니다. 비공식 CLI 스스로 "이용약관 위반에 해당할 수 있"다고 경고한다 [S11]. 쓰지 않는다 |

권장: (가)를 기본으로 하고, 실사용 중 IP가 자주 바뀌면 (나)로 옮긴다. design.md 위험표의 판단과 같다.

### Q4. 토큰

- 유효 시간: 응답의 `expires_in`(초)이 기준이다. 스펙 예시값은 `86400`(24시간)이다 [S5]. 실제 발급값은 실측하지 않아 미확인이다. 2차 자료 하나는 86400초라고 쓴다 [S12]. 구현은 상수가 아니라 응답값을 따라야 한다.
- refresh token은 없다. 만료되면 같은 엔드포인트로 다시 발급한다 [S5].
- 재발급 동작 (원문): "client 당 유효한 access token 은 1 개입니다. 재발급 시 이전에 발급된 token 은 즉시 무효화됩니다." 이전 토큰으로 호출하면 401 `token-revoked` [S5, S4].
- 웹소켓: 연결 중에도 토큰을 다시 검증한다. 만료·재발급·삭제로 무효가 되면 `auth-revoked` 프레임 뒤 연결을 닫는다. "토큰이 만료되면 연결도 종료되므로, 만료 전에 새 토큰을 발급받아 재연결하세요" [S6].
- 여러 기기·프로세스: FAQ 원문 "국내·해외 주식을 별도 프로세스로 운영하더라도 하나의 토큰을 공유해 사용하세요. 프로세스마다 토큰을 따로 발급하면 서로를 무효화합니다." [S3]. 같은 키를 두 기기(또는 이 앱 + 다른 도구, 예를 들어 토스를 연결한 다른 AI 앱이나 `toss-smoke`)에서 쓰면 서로의 토큰과 웹소켓을 계속 끊는다.
- 키 재발급: 비공식 CLI 문서에 따르면 재발급하면 기존 키가 즉시 폐기되고 시크릿은 다시 볼 수 없다 [S11, S14]. 사용자당 키를 여러 개 만들 수 있는지는 미확인이다.
- 토큰 발급 자체도 `AUTH` 그룹 초당 5회 한도를 받는다 [S4].

설계 결론: 토큰은 앱 코어의 한 곳만 발급한다 (T-02). 401 `token-revoked`가 오면 "다른 곳에서 같은 키를 쓰는 중"으로 보고, 바로 재발급하지 말고 한 번만 재발급한다. 다시 revoked가 오면 멈추고 사용자에게 알린다. 그러지 않으면 두 프로세스가 서로를 무한히 무효화한다.

### Q5·Q14. 거래내역 API (`GET /api/v1/transactions`)

**제공 범위** [S5]

- 대상: 매매, 입출금, 환전, 배당, 이자, 수수료, 증권 입출고. KR·US, 주식·옵션·채권을 한 목록으로 준다. 매매 수수료는 따로 행이 되지 않고 매매 행의 `commission`에 들어간다.
- 조회 기간: `from` 미지정이면 "하한 없음", `to` 미지정이면 "상한 없음 (미결제 거래 포함)". 날짜 필터는 결제일(`settlementDate`) 기준이고, 미결제는 예상 결제일에 적용된다. 실제로 몇 년 전까지 데이터가 있는지는 미확인이다. (참고: 주문 목록은 FAQ에 "계좌 개설 이후 전체 기간"이라고 적혀 있다 [S3]. 거래내역에는 같은 문장이 없다.)
- 페이징: `cursor` + `limit`(1–100, 기본 20). "반환 건수가 limit보다 적어도 다음 페이지가 있을 수 있으므로 hasNext를 확인." 계좌나 필터를 바꾸면 커서를 버린다. 커서 오류 400이 오면 첫 페이지부터 다시 조회한다.
- 정렬: 미결제가 먼저, 그다음 결제 완료. 각 그룹 안에서는 최신순이다.
- "거래내역에 해당하는 주문의 식별자는 제공하지 않습니다." 거래 고유 ID도 없다.
- 이 API는 2026-10-29과 11-04 사이에 버전 번호 1.2.28을 유지한 채 추가됐다 (Q9).

**미결제 → 결제** [S5]

| 상품 | 미결제 행 | 결제 완료 행 |
|---|---|---|
| 국내주식 | 같은 주문일·종목·매매방향의 체결 합산 | 체결가 등 결제 기준이 같은 내역 합산. "미결제와 합산 기준이 다름" |
| 미국 주식 | 주문별 체결 합산 | 주문별 체결 합산 |
| 미국 옵션·채권 | 체결별 | 체결별 |

- "국내주식은 미결제 한 건이 결제 후 여러 건의 결제 내역으로 바뀔 수 있습니다."
- 미결제 행은 `filledPrice`, `exchangeRate`, `transactionTime`이 null이다. `netAmount`는 결제 전 계산값이다.
- `settled=false`이면서 예상 결제일이 오늘이거나 과거일 수 있다. 결제 여부는 `settled`로만 판단한다.
- 미결제로 나오는 것은 `BUY`·`SELL`뿐이다. 나머지는 항상 결제된 상태로 나온다.

**취소 거래** [S5]

- `isCancellation`: "국내 거래의 취소 여부. true이면 거래를 취소한 내역이며, **원거래는 목록에서 제외됩니다**." 유형은 원래 유형을 유지한다. 입금 취소는 돈이 나가고 출금 취소는 돈이 들어온다.
- "거래를 취소하는 대신 반대 방향의 새 거래로 처리한 경우에는 false이며, 기존 거래와 새 거래가 함께 조회됩니다. 모든 취소 방식을 나타내는 값은 아닙니다."
- 금액은 모두 부호 없는 절대값이고, 방향은 유형과 `isCancellation`으로 정한다.
- 중요한 결과: **이미 저장한 결제 완료 행이 나중에 목록에서 사라질 수 있다** (국내 취소). "결제 내역은 추가만 한다"는 가정은 틀린다.

**액면분할·종목 코드 변경** [S5, S3]

- `isinCode`는 "거래 시점의 ISIN 이므로, 국내 종목의 액면분할·병합 이후에는 `GET /api/v1/stocks/{symbol}` 의 현재 값과 다를 수 있습니다."
- 분할·병합·코드 변경을 나타내는 거래 유형은 없다. 주식배당은 `SECURITIES_TRANSFER_IN`(수량 있음)으로 나온다. 분할·병합이 입출고 행으로 나오는지는 미확인이다.
- 과거 거래의 `symbol`이 거래 당시 코드인지 현재 코드인지는 문서에 없다. 미확인.
- 상장폐지 종목은 시세 조회가 `404 stock-not-found`다 [S3]. 거래내역에 남은 폐지 종목의 이름은 이 API로 얻을 수 없다 (거래내역에는 종목명이 없다).
- 결론: 분할·코드 변경은 이 API만으로 처리할 수 없다. 보유 주식 API의 현재 수량·평균가를 기준으로 삼고, 거래내역은 현금 흐름과 기록용으로 쓰는 것이 안전하다. 분할 처리 규칙은 R-11에서 정한다.

**중복 없이 저장하는 기준 (Q14, 추천)**

고유 ID가 없고, 결제 후 합산 단위가 바뀌고, 취소로 행이 사라지므로 "행 단위 영구 키"는 만들 수 없다. 대신 다음을 제안한다.

1. 미결제 행(`settled=false`)은 저장하지 않거나 별도 임시 테이블에 두고, 동기화 때마다 통째로 교체한다 (tasks.md A-07과 같음).
2. 결제 완료 행은 **결제일 단위 창을 다시 받아 맞추는 방식**으로 저장한다.
   - 내용 키 = (`settlementDate`, `transactionType`, `marketCountry`, `currency`, `assetClass`, `symbol`, `isinCode`, `quantity`, `filledPrice`, `amount`, `commission`, `tax`, `netAmount`, `exchangeRate`, `transactionTime`, `isCancellation`) 전체의 정규화 해시.
   - 같은 내용 키가 한 창 안에 여러 번 나올 수 있으므로 (같은 시각·같은 금액의 합산 행) 키별 **개수**를 저장한다. 즉 멀티셋으로 맞춘다.
   - 동기화할 때 최근 N일(제안: 결제일 기준 최근 14일)을 다시 받아, 그 창의 멀티셋을 서버 값으로 바꾼다. 줄어든 행에는 삭제 표시(사라진 시각)를 남기고 지우지 않는다. 원본 보존 요구(architecture.md 5장)와 맞는다.
   - 창보다 오래된 결제 완료 행은 바뀌지 않는다고 보고 다시 받지 않는다. 다만 하루 한 번 또는 주 한 번 전체 기간을 다시 받아 멀티셋이 같은지 검사하고, 다르면 경고한다. 오래된 행이 바뀌는 경우가 있는지는 미확인이므로 이 검사가 그것을 알려 준다.
3. 정규화된 거래 기록(평균가, 실현 손익 계산용)은 이 원본 멀티셋에서 다시 만든다. 원본이 바뀌면 다시 계산한다.

이 기준이 틀렸다고 판명될 조건: (a) 결제 완료 행의 `transactionTime`이나 금액이 시간이 지나며 바뀌는 사례가 실제 계좌에서 관측될 때. (b) 14일보다 오래된 거래가 취소되는 사례가 관측될 때. 그러면 창을 늘린다. (c) 토스가 거래 ID를 추가하면 그 ID로 바꾼다.

### Q6. 보유 주식 금액과 토스 앱 화면의 일치 조건

스펙에서 확인되는 것 [S5]:

- 종목별 금액은 거래 통화 기준이다. 요약 금액(`totalPurchaseAmount`, `marketValue`, `profitLoss`, `dailyProfitLoss`)은 통화별 합산(`krw`, `usd`)이다. `usd`는 해외 종목이 없으면 null이다. 원화 환산 합계는 주지 않는다.
- 비율 필드(`rate`, `rateAfterCost`, 일간 `rate`)만 "전체 자산을 현재 환율로 원화 환산한 기준"이다. 어떤 환율(`rate` 매수 환율인지 `midRate` 매매기준율인지)인지, 언제 시각인지는 미확인이다.
- 수수료·세금: `amount`(시장 평가)와 `amountAfterCost`("세금/수수료 공제 후 평가금액"), `profitLoss.amount`와 `amountAfterCost`가 함께 온다. 종목별 `cost.commission`, `cost.tax`(없으면 null)가 따로 있다. 이 비용이 "지금 팔면 낼 예상 매도 비용"인지 "매수 때 낸 비용"인지는 문서에 없다. 미확인.
- 대상은 국내·미국 주식뿐이다. 해외 옵션·채권은 빠진다.
- `symbol` 필터를 주면 요약도 그 종목 기준으로 다시 계산한다.

토스 앱 화면이 `amount` 기준인지 `amountAfterCost` 기준인지, 앱의 원화 총액이 어떤 환율을 쓰는지는 공식 문서에 없다. **미확인.** T-13(실제 키로 대조, ACC-6)에서 확인해야 한다. 대조할 항목: 종목별 평가금액·손익·손익률, 요약 손익률, 일간 손익을 앱 화면과 `amount`/`amountAfterCost` 양쪽에 비교한다. 같은 시각의 `GET /api/v1/exchange-rate`(`rate`, `midRate`)로 원화 환산해서 앱 총액과 비교한다.

### Q7. 주문 API

**주문 유형과 시장별 제약** [S5, S3]

| 조합 | KR | US |
|---|---|---|
| `LIMIT` + `DAY` | 가능. 가격은 정수 원, 호가 단위에 맞아야 함 (어긋나면 400과 함께 `data.tickSize`, `nearestPrices`) | 가능. $1 미만 소수 4자리, $1 이상 소수 2자리 (넘는 자릿수는 절삭) |
| `MARKET` + `DAY` | 가능 | 가능 |
| `LIMIT` + `CLS` (LOC) | 불가 | 가능 |
| `OPG` (시가단일가) | REST 스펙: 가능 (`LIMIT`/`MARKET`), 장전 사전접수 외 시간은 원장에서 거절될 수 있음 | 불가 |
| 금액 주문 `orderAmount` (달러) | 불가 | `MARKET`만. 정규장 시작부터 종료 1시간 전까지 |
| 소수점 `quantity` | 불가 | `MARKET` + `SELL`만, 소수 6자리까지, 같은 시간 제약 |

- 같은 종목의 반대 방향 체결 대기 주문이 있으면 409 `opposite-pending-order-exists`. **국내 주식 일반 주문에만 적용**되고 미국 주식과 조건주문에는 적용되지 않는다 [S3].
- 자동 환전 없음. 달러가 부족하면 422 `insufficient-buying-power` [S3].
- 국내 주문은 통합(SOR) 모드만 지원한다. 거래소를 지정할 수 없다 [S3].
- 정정: KR은 `quantity` 필수(양의 정수), US는 가격만 바꿀 수 있다 (`quantity`를 주면 400 `us-modify-quantity-not-supported`). 정정·취소 응답의 `orderId`는 "새로 발급된 주문 식별자. 원주문의 orderId 와 다릅니다" [S5]. 원주문과 새 주문을 잇는 필드는 응답에 없으므로 앱이 저장해야 한다.
- Open API로 낼 수 없는 호가 유형(장전·장후 시간외 종가 등)의 주문은 주문 목록·상세 조회에 나오지 않는다. 앱·WTS로 낸 다른 주문은 나온다 [S5, S3].
- 매수·매도 건수 한도는 따로 없다. `ORDER` 그룹 초당 10회는 매수·매도 합산이다 [S3].
- 문서 불일치: AsyncAPI의 주문 이벤트 스키마는 `OPG`를 "현재는 지원하지 않습니다"라고 쓴다 [S6]. REST 스펙은 국내 전용으로 지원한다고 쓴다. 1차 범위에서 `OPG`를 쓰지 않으면 영향이 없다.

**1억 원 이상 확인 값** [S5]

- `confirmHighValueOrder`(기본 `false`). "1억원 이상의 주문 시 true가 아니면 `400 confirm-high-value-required`". 오류 `data`는 `{"field":"confirmHighValueOrder","limits":{"threshold":"100000000","currency":"KRW"}}`다.
- 주문 생성, 주문 정정, 지정가 조건주문 생성·수정에 적용된다.
- 30억 원 초과는 플래그와 무관하게 422 `max-order-amount-exceeded` (`limits.maxAmount` = `3000000000` KRW).
- 금액 계산식(시장가 주문의 기준 가격, 정정 시 남은 수량인지 전체 수량인지)과 달러 주문의 환율은 문서에 없다. 미확인 (Q13).
- 1억 원 미만 주문에 `true`를 보내면 어떻게 되는지 문서에 없다. 미확인.

**중복 방지 키 `clientOrderId`** [S5, S3]

- 최대 36자, `^[a-zA-Z0-9\-_]+$`. 서버가 자동 생성하지 않는다. 주지 않으면 멱등성이 없다.
- "동일 값으로 재요청 시 이전 주문 결과를 그대로 재반환합니다." 유효 시간은 **10분**이다. 10분이 지나면 같은 값이어도 새 주문이 된다.
- 첫 요청이 처리 중이면 409 `request-in-progress`. 같은 키로 내용을 바꾸면 422 `idempotency-key-conflict`.
- 첫 요청이 **오류로 끝났을 때** 같은 키로 다시 보내면 같은 오류가 다시 오는지, 새로 처리되는지는 문서에 없다. 미확인. 그래서 design.md의 "고액 확인 거절 후 새 키로 다시 보낸다"가 안전한 쪽이다.
- 주문 목록·상세·웹소켓 주문 이벤트 어디에도 `clientOrderId`가 없다 (`Order` 스키마 필드: `orderId, symbol, side, orderType, timeInForce, status, price, quantity, orderAmount, currency, orderedAt, canceledAt, execution`). 주문 생성 응답에만 있다.

**전송 후 응답을 못 받았을 때 확인 방법**

1. 10분 안: 같은 `clientOrderId`와 같은 본문으로 다시 보낸다. 성공 응답이면 첫 주문의 `orderId`가 온다. 409 `request-in-progress`면 잠시 뒤 다시 보낸다 [S3].
2. 10분이 지났거나 서버가 점검 중이었으면 (`maintenance` 예시의 `retryAfterSeconds`가 600, 즉 10분이다): 다시 보내지 않는다. `GET /api/v1/orders?status=OPEN&symbol=…`(전량 반환)과 `status=CLOSED&symbol=…&from=오늘`을 조회하고, `side`, `orderType`, `price`, `quantity`/`orderAmount`, `orderedAt ≥ 전송 시각 − 여유`로 대조한다. `clientOrderId`로 찾을 수 없기 때문이다.
3. 웹소켓 `personal:order`가 연결돼 있었다면 그 사이의 `PENDING` 이벤트도 같은 기준으로 대조한다. 끊긴 구간의 이벤트는 다시 오지 않는다 [S6].
4. 같은 내용의 주문이 둘 이상 대조되거나 하나도 안 되면 "확인 불가"로 두고 같은 종목의 새 주문을 막는다 (ORD-2). 사용자가 토스 앱에서 확인하도록 안내한다.

**오류 코드 전체 목록과 재시도 여부**

출처: overview 오류표 [S4]와 OpenAPI JSON 응답 예시 [S5]. 두 목록은 조금 다르다: `fractional-quantity-outside-regular-hours`, `us-modify-quantity-not-supported`, `unsupported-date`는 스펙에만 있다. `order-limit-exceeded`, `unsupported-content-type`, `edge-rate-limit-exceeded`는 overview에만 있다. "재시도" 열은 문서 문구를 근거로 한 이 조사의 판단이다. 주문 계열(생성)에서 "같은 키"는 10분 안에 같은 `clientOrderId`와 같은 본문으로 다시 보내는 것을 뜻한다. `clientOrderId` 없이 보낸 주문은 어떤 오류에서도 다시 보내지 않는다.

| HTTP | 코드 | 뜻 | 재시도 | A-05 분류 |
|---|---|---|---|---|
| 400 | `invalid-request` | 요청 형식·값 오류 (호가 단위, 수량, 필수값, 커서 등). `data`에 힌트 | 아니오. 커서 오류만 커서 없이 처음부터 | 버그 또는 사용자 수정 |
| 400 | `confirm-high-value-required` | 1억 원 이상인데 플래그 없음 | 사용자가 금액을 다시 확인한 뒤 **새 키**로 | 사용자 수정 |
| 400 | `account-header-required` | 계좌 헤더 누락 | 아니오 | 버그 |
| 400 | `unsupported-ranking-duration`, `unsupported-symbol`, `unsupported-market`, `unsupported-date` | 지원하지 않는 조회 조건 | 아니오 | 버그 |
| 400 | `us-modify-quantity-not-supported` | 미국 정정에 수량 포함 | 아니오 | 버그 |
| 401 | `invalid-token` | 토큰 무효·형식 오류 | 재발급 1회 후 1회 | 버그 가능성 |
| 401 | `expired-token` | 만료 | 재발급 후 같은 요청 (주문은 같은 키) | 기다리면 됨 |
| 401 | `token-revoked` | 다른 곳에서 재발급 | 재발급 1회 후 1회. 다시 오면 중단하고 알림 | 사용자 수정 (키 중복 사용) |
| 401 | `login-user-not-found` | 토큰의 로그인 정보 없음 | 아니오 | 사용자 수정 |
| 401 | `edge-blocked` | Authorization 머리말 없음 | 아니오 | 버그 |
| 401 (토큰) | `invalid_client` | client_id/secret 오류 (`error_description`에 어느 쪽인지) | 아니오 | 사용자 수정 |
| 400 (토큰) | `invalid_request` 등 OAuth 오류 | 요청 형식 | 아니오 | 버그 |
| 403 | `ip-not-allowed` / 토큰 `access_denied` | 허용 IP 아님 | 아니오 (REL-1) | 사용자 수정 |
| 403 | `forbidden` | 권한 부족 | 아니오 | 사용자 수정 (원인 미확인) |
| 403 | `edge-blocked` | 앞단 차단 (요청값 이상) | 아니오 | 버그 |
| 404 | `edge-blocked` | 지원하지 않는 경로 | 아니오 | 버그 |
| 404 | `stock-not-found`, `exchange-rate-not-found`, `sector-not-found` | 대상 없음 | 아니오 | 데이터 없음 |
| 404 | `account-not-found` | 계좌 없음 | 아니오. 계좌 목록 다시 조회 | 사용자 수정 |
| 404 | `order-not-found`, `conditional-order-not-found` | 주문 없음. 조건주문 수정에서는 "이미 반영됨"의 신호 [S3] | 아니오 | 상황별 |
| 409 | `request-in-progress` | 같은 키 처리 중 | 예, 잠시 뒤 같은 키 | 기다리면 됨 |
| 409 | `opposite-pending-order-exists` | 국내 반대 방향 미체결 존재 | 아니오 | 사용자 수정 |
| 409 | `already-filled`, `already-canceled`, `already-modified`, `already-rejected` | 정정·취소 대상이 이미 종료 | 아니오. 주문 상태 다시 조회 | 상태 변화 |
| 409 | `already-processing` | 정정·취소 처리 중. `data.retryAfterSeconds` | 예, 그 시간 뒤 | 기다리면 됨 |
| 414 | `edge-blocked` | URI 너무 김 | 아니오 | 버그 |
| 415 | `unsupported-content-type` | `application/json` 아님 | 아니오 | 버그 |
| 422 | `insufficient-buying-power` | 매수가능금액 부족 (자동 환전 없음) | 아니오 | 사용자 수정 |
| 422 | `order-hours-closed` | 접수 불가 시간. `data.retryAfterAt` | 자동 재전송 안 함. 제안 만료로 처리 | 기다리면 됨 |
| 422 | `stock-restricted` | 종목 거래 제한 | 아니오 | 사용자 수정 |
| 422 | `price-out-of-range` | 상·하한가 밖 | 아니오 | 사용자 수정 |
| 422 | `order-type-not-allowed` | 지금 쓸 수 없는 호가 유형 | 아니오 | 사용자 수정 |
| 422 | `prerequisite-required` | 약관 동의·교육·위험 고지 등 미충족 | 아니오. 토스 앱에서 절차 안내 | 사용자 수정 |
| 422 | `market-not-supported-for-stock` | 종목·시장 조합 불가 (KR) | 아니오 | 버그 또는 데이터 |
| 422 | `investor-exchange-not-integrated` | 투자자지시 거래소가 통합(SOR) 아님 (KR) | 아니오. 설정 변경 안내 | 사용자 수정 |
| 422 | `amount-order-outside-regular-hours`, `fractional-quantity-outside-regular-hours` | 금액·소수점 주문 시간 밖. `data.orderableHours` | 아니오 | 사용자 수정 |
| 422 | `modify-restricted`, `cancel-restricted` | 정정·취소 제한 | 아니오 | 사용자 수정 |
| 422 | `insufficient-sellable-quantity` | 매도 가능 수량 부족 | 아니오 | 사용자 수정 |
| 422 | `order-limit-exceeded` | 주문 설정 한도 초과 (overview에만 있음) | 아니오 | 사용자 수정 |
| 422 | `duplicate-conditional-order`, `condition-already-met` | 조건주문 제약 | 아니오 | 사용자 수정 |
| 422 | `idempotency-key-conflict` | 같은 키에 다른 내용 | 아니오 | 버그 |
| 422 | `account-restricted` | 계좌 상태가 주문 불허 (RIA·연금, 사고 계좌 등) | 아니오 | 사용자 수정 |
| 422 | `max-order-amount-exceeded` | 30억 원 초과 | 아니오 | 사용자 수정 |
| 429 | `rate-limit-exceeded`, `edge-rate-limit-exceeded` | 속도 제한 | 예, `Retry-After` 뒤 (주문은 같은 키로) | 기다리면 됨 |
| 500 | `internal-error` | 일시 장애. **주문이 접수됐는지 모름** | 주문: 같은 키로만. 조회: 백오프 | 기다리면 됨 |
| 500 | `maintenance` | 점검. `data.retryAfterSeconds` (예시 600) | 조회: 그 뒤. 주문: 10분이 지나면 다시 보내지 않고 대조 | 기다리면 됨 |
| - | 네트워크 단절·시간 초과 | 응답 없음 | 주문: 위 "응답을 못 받았을 때" 절차 | 기다리면 됨 |

인증 없이 실제로 호출해 본 결과 (2026-11-11, 키 없음):

```
$ curl -sS -D - 'https://openapi.tossinvest.com/api/v1/prices?symbols=005930'
HTTP/2 401
content-type: application/json
...
{"error":{"referenceId":"0.c7bfc017.1794385776.661c1cc","code":"edge-blocked","message":"Authorization 헤더가 전달되지 않았습니다."}}
```

앞단(edge) 오류는 본문에 `requestId` 대신 `referenceId`를 준다. 오류 파서는 둘 다 받아야 한다.

### Q8. 웹소켓

출처: AsyncAPI 1.2.3과 overview [S6, S4].

| 항목 | 값 |
|---|---|
| 주소 | `wss://openapi-ws.tossinvest.com/ws/v1`, TLS 필수, 핸드셰이크에 `Authorization: Bearer` |
| 동시 연결 | **계정당 2개**. 넘으면 새 연결이 수락되고 가장 오래된 연결이 닫힌다 |
| 구독 수 | 연결당 100건 (`codes` 합산, 채널×종목, 주문 채널의 `accountSeq` 포함). 초과는 `too-many-topics` |
| 선언 빈도 | 초당 5회. 초과는 `rate-limit-exceeded` 프레임 (`Retry-After` 없음, 약 1초 대기) |
| 구독 모델 | 배열 하나가 전체 구독 (full-replace). `[]`은 전체 해제 |
| 연결 유지 | 클라이언트 수신이 180초 없으면 서버가 닫는다. 서버가 보내는 데이터는 타이머를 리셋하지 않는다. 텍스트 `PING`을 60초마다 권장 |
| 종료 신호 | `server-shutdown`(배포), `auth-revoked`(토큰 만료·재발급·삭제, 허용 IP 제외, 계좌 부적격). 그 외(유휴 초과, 수신 지연, 연결 수 초과로 밀려남)는 close code 없이 끊길 수 있다 |
| 재연결 | 지수 백오프 1s→2s→4s… + jitter. 이전 연결을 먼저 닫는다 ("이전 연결이 남아 있으면 … 끊김이 반복될 수 있습니다"). 재연결 후 구독을 다시 선언한다 |
| 초기 스냅샷 | 호가는 구독 직후 스냅샷을 보내지 않는다. REST로 먼저 조회 |

**본인 주문 이벤트(`personal:order`)의 신뢰성**

- 전달 보장은 LOSSLESS다. 하지만 "연결 세션 내부에 한정되어 끊긴 구간의 이벤트는 다시 전달되지 않으므로, 재연결 후 다시 선언하고 `GET /api/v1/orders` 로 주문 상태를 재동기화"해야 한다.
- 클라이언트 수신이 2초 이상 막히면 서버가 끊는다. 수신 루프는 처리와 분리해 바로 큐에 넣어야 한다.
- 이벤트 종류: `PENDING, PARTIAL_FILL, FILL, CANCELING, CANCELED, REPLACING, REPLACED, REJECTED, CANCEL_REJECTED, REPLACE_REJECTED`. 모르는 값을 허용하라고 명시한다.
- `data.order`는 주문 상세와 같은 모양이지만 `execution.filledAt`이 없다. 이벤트 ID, 순번, `clientOrderId`가 없다. 중복·순서 역전을 감지할 수단이 없으므로, 이벤트는 "다시 조회하라는 신호"로 쓰고 상태의 진실은 REST 주문 상세로 정하는 것이 안전하다.
- 국내·미국 주식 주문만 전달된다. 본인 종합매매·활성 계좌만 구독된다.
- 시세 채널(`trade`, `orderbook`)은 LOSSY다. 수신이 밀리면 중간 프레임이 빠지고 유실을 알 수 있는 순번이 없다.

토큰이 24시간(예시값)마다 만료되면 연결도 끊기므로, 만료 전에 새 토큰을 받고 계획된 재연결을 해야 한다. 이때 REST와 웹소켓이 같은 토큰을 쓰므로 재발급 순간 기존 연결이 `auth-revoked`로 닫힌다. 순서: 새 토큰 발급 → 기존 연결 닫기 → 새 연결 → 재선언 → 주문 재조회.

### Q9. 스펙 변경 빈도, 하위 호환, 공지 위치

**공식 버전 목록** (`/openapi-docs/versions.json`) [S7]

| 버전 | 공개일 | 상태 | 마지막 패치 |
|---|---|---|---|
| 1.0 | 2026-06-01 | archived | 5 |
| 1.1 | 2026-06-06 | archived | 8 |
| 1.2 | 2026-07-03 | current | 28 |

1.0과 1.1은 `/openapi-docs/v1.0/openapi.json`, `/v1.1/`에 보관돼 있다. 패치 단위(예: 1.2.27)는 따로 보관되지 않는다 (`/openapi-docs/1.2.27/openapi.json` 등은 404).

**패치 변경 이력 (2차 자료로 재구성)**: 비공식 CLI 저장소가 공식 스펙을 자동으로 받아 커밋해 둔 스냅샷 22개(2026-06-11 ~ 2026-11-04)를 내려받아 비교했다 [S11]. 마지막 스냅샷은 저장소의 `docs/toss-openapi-1.2.28.json`과 정규화 해시가 같다 (그래서 이 스냅샷들이 공식 파일의 사본이라고 볼 수 있다).

| 관찰 | 근거 |
|---|---|
| 5개월 동안 내용이 바뀐 스냅샷 22개. 1.2 패치는 1.2.1(07-06)에서 1.2.28(11-04)까지 약 4개월에 27번 올라갔다. 대략 주 1–2회 | 스냅샷 버전·해시 비교 |
| **같은 버전 번호로 내용이 바뀐 경우가 있다.** 1.2.14는 4가지 내용이 있었다. 1.2.13은 같은 번호에서 `GET /api/v1/stocks/all`이 추가됐다. 1.2.28은 10-29 → 11-04 사이에 `GET /api/v1/transactions`가 추가됐다 | 스냅샷 해시와 엔드포인트 비교 |
| 엔드포인트는 21개 → 43개로 늘었고 삭제된 엔드포인트는 없다. 요청 파라미터가 새로 필수가 되거나 삭제된 경우는 없다 | 자동 비교 스크립트 |
| 응답 enum 값 삭제: 1.2.2에서 `ConditionalOrderDetailResponse.status`의 `CANCELED`가 빠졌다 (취소된 조건주문은 REST에서 사라지고 웹소켓에만 `CANCELED`로 나온다) | 비교 결과와 AsyncAPI 설명 |
| 응답 필드가 새로 필수가 됨: 1.2.24에서 `ConditionalOrderCondition.orderSide` | 비교 결과 |
| 동작 변경: 1.2.5에서 `GET /api/v1/orders?status=CLOSED`가 오류 대신 페이징 응답을 주기 시작했다 | 비공식 CLI CHANGELOG [S11] |
| 1.2.9에서 소수점·금액 주문 접수 시간이 "정규장 종료 1시간 전까지"로 문서화됐다 | 같음 |

결론: 요청 쪽 하위 호환을 깬 변경은 이 비교 범위에서 찾지 못했다. 응답 쪽은 enum 값 삭제와 동작 변경이 있었다. 스펙 곳곳에 "클라이언트는 unknown enum 값을 허용하도록 구현해야 합니다"라고 적혀 있어, 토스는 enum 추가를 호환 변경으로 본다 [S5]. 이 비교는 스키마 구조만 봤고 설명 문구의 의미 변화는 보지 않았다.

**변경 공지 위치**: 약관 제15조는 "회사 홈페이지 또는 API홈페이지에 게시"한다고 한다 [S1]. 하지만 developers.tossinvest.com 문서, `llms.txt`, FAQ, Open API 소개 페이지에서 변경 이력·공지 페이지를 찾지 못했다 [S9, S2]. 알 수 있는 것은 `versions.json`, 파일의 `Last-Modified` 머리말(조사일 기준 모든 문서가 `Wed, 04 Nov 2026 01:48:38 GMT`), 파일 내용뿐이다. 공식 변경 공지 채널은 미확인이다.

### Q10. 모의 환경·테스트 계좌

- 스펙의 서버는 `https://openapi.tossinvest.com` 하나다 [S5]. `llms.txt`의 서버 목록도 REST·웹소켓 각각 하나다 [S9].
- 공식 문서·FAQ·약관에 모의투자, 샌드박스, 테스트 계좌라는 말이 없다 (스펙 전문 검색 결과 0건).
- 2차 자료도 "모의투자 환경이 따로 보이지 않습니다"라고 쓴다 (2026-08-04 확인 기준) [S12].
- 결론: 공식 모의 환경은 찾지 못했다. 주문 경로는 녹화 응답과 가짜 서버로 시험하고, 마지막 확인은 소액 실주문(O-15)뿐이다. 토스 주문에는 "검증만 하고 접수하지 않는" 모드도 문서에 없다.

### Q11. 주문 전 계좌가 갖춰야 할 조건

| 조건 | 내용 | 근거 |
|---|---|---|
| 계좌 종류 | 종합매매(`BROKERAGE`)만 노출. 자녀계좌 불가. 연금·RIA 계좌 주문은 `account-restricted` | [S5] |
| 나이 | 만 19세 미만 불가 | [S1, S2] |
| Open API 가입 | 약관 동의, 본인인증, 계좌인증 후 키 발급 | [S1] 제6조 |
| `prerequisite-required` | "약관 동의·교육 이수·위험 고지 등 사전 자격 요건을 충족하지 않았습니다." FAQ 예: 미국 레버리지·인버스 ETF는 "투자자 성향 등록·위험 고지·약관 동의 등"이 필요하고, "Open API 로는 등록 절차를 진행할 수 없으므로 토스증권 앱에서 해당 종목 구매 화면에 진입해 안내되는 등록 절차를 마친 뒤" 다시 시도 | [S4, S3] |
| `investor-exchange-not-integrated` | 국내 주문·정정에서 "투자자지시 거래소가 통합(SOR) 으로 설정되어 있어야 주문할 수 있습니다." Open API는 SOR 모드만 지원 | [S5, S3] |
| 해외 주식 거래 신청 | 별도 신청이 필요한지, 없으면 어떤 오류가 나는지 문서에 없다. 미확인 | |
| 달러 | 자동 환전이 없고 API로 환전할 수 없다. 미국 주식을 사려면 앱에서 미리 환전해 둬야 한다 | [S3, S2] |
| 종목별 제한 | 매수 유의사항 API(`/stocks/{symbol}/warnings`)로 정리매매·투자경고 등을 미리 확인. 거래정지는 종목 정보의 `krxTradingSuspended`·`nxtTradingSuspended` | [S5, S3] |

사용자가 미리 할 일 (T-00 체크리스트 후보):
1. 토스증권 앱·WTS에서 투자자지시 거래소 설정을 "통합(SOR)"로 둔다. 메뉴 위치는 문서에 없다 (미확인).
2. 거래할 상품군(미국 주식, 레버리지·인버스 ETF 등)을 앱에서 한 번 매수 화면까지 들어가 투자자 성향·위험 고지·약관 절차를 마친다.
3. 미국 주식을 살 달러를 미리 환전해 둔다.
4. 허용 IP를 등록한다.

`prerequisite-required`가 정확히 어떤 요건마다 나는지의 전체 목록은 문서에 없다. 미확인.

### Q12. 예수금과 총자산

- 예수금(현금 잔고) API는 없다. 계좌 API는 계좌 목록뿐이고, 보유 주식 API는 주식만 준다 [S5].
- `GET /api/v1/buying-power?currency=KRW|USD` → `cashBuyingPower`: "현금 기반 매수 가능 금액 (미수 미발생 기준). 순수 현금으로 매수할 수 있는 금액." KRW는 정수, USD는 소수 [S5]. 미체결 매수 주문이 잡아 둔 금액, 매도 미결제 대금, 환전 대기 금액이 여기에 어떻게 반영되는지는 미확인이다.
- 거래내역의 `netAmount`도 "예수금 변동과 같지 않을 수 있습니다" [S5]. 그래서 거래내역을 더해 잔고를 재구성하는 방법도 정확하지 않다.
- 결론: 원화·달러 현금 잔고를 정확히 알 수 있는 공식 경로는 **없다**. 화면에는 "매수가능금액(원)", "매수가능금액(달러)"이라는 이름으로 `cashBuyingPower`를 보여 주고, 총자산은 "보유 평가액 + 매수가능금액(근사)"으로 표시하며 근사라고 밝힌다. 토스 앱의 예수금·총자산과 얼마나 다른지는 T-13에서 실측한다.

### Q13. 달러 주문의 1억 원 판정 환율

공식 문서에 없다. **미확인.** 관련 사실:

- 오류 `data.limits`는 `{"threshold":"100000000","currency":"KRW"}`만 준다. 판정에 쓴 환율이나 환산 금액은 주지 않는다 [S5].
- 환율 API는 `rate`(매수 환율), `midRate`(매매기준율), 1분 유효 구간을 준다. "참고용 표시 환율. 실제 주문 시 적용되는 거래 환율과 다를 수 있습니다." [S5]
- 토스의 환전 안내: 매매기준율은 고시 (매수 환율 + 매도 환율) / 2이고, "환전은행의 실시간 매매 기준 환율을 적용"한다 [S10 하단 고지]. 이것이 1억 판정에도 쓰인다는 근거는 없다.

권장 처리: 앱은 환율 API의 `rate`와 `midRate` 중 큰 값으로 원화 환산하고, 환산액이 1억 원의 95% 이상이면 고액 확인을 받고 `confirmHighValueOrder: true`를 붙인다. 그래도 400 `confirm-high-value-required`가 오면 design.md 규칙대로 같은 확인을 거쳐 새 키로 다시 보낸다. 1억 원 미만에 `true`를 붙였을 때의 동작이 미확인이므로, 이 방식은 처음 실주문(O-15) 때 확인해야 한다.

## 3. 선택지 비교와 추천

### 3.1 거래내역 중복 제거 방식

| 방식 | 국내 결제 후 행 분할 | 국내 취소로 행 사라짐 | 같은 내용 행 여러 개 | 구현 난도 | 판단 |
|---|---|---|---|---|---|
| A. 결제 완료 행을 내용 해시 하나로 "있으면 건너뛰기" | 처리됨 | 놓침 (사라진 원거래가 계속 남음) | 둘째 행을 잃음 | 낮음 | 틀림 |
| B. 매번 전체 기간을 받아 통째로 교체 | 처리됨 | 처리됨 | 처리됨 | 낮음 | 호출량이 계좌 나이에 비례해 늘고, 원본 이력이 없다 |
| C. 최근 N일 창을 다시 받아 멀티셋으로 맞추고, 사라진 행은 삭제 표시 + 주기적 전체 검사 | 처리됨 | 처리됨 (창 안에서) | 처리됨 | 중간 | **추천** |
| D. 주문 목록과 거래내역을 이어 붙여 주문 ID로 묶기 | 연결 키가 없어 추정이 필요 | - | - | 높음 | 쓰지 않음 |

추천: **C**. 이유: 문서가 밝힌 세 가지 동작(분할, 취소 시 원거래 제외, 합산 행)을 모두 다루면서 호출량을 일정하게 유지하고 원본 이력을 남긴다. 30분마다 14일 창을 받는 비용은 `TRANSACTION` 초당 5회 한도에 비해 작다.
틀렸다고 판명될 조건: Q5 끝에 적은 (a)–(c).

### 3.2 응답 불명 주문의 확인

| 방식 | 10분 안 | 10분 뒤 | 판단 |
|---|---|---|---|
| A. 같은 `clientOrderId`로 다시 보내기만 | 확정됨 | 새 주문이 생김 (중복 주문 위험) | 10분 뒤에는 위험 |
| B. 주문 조회로만 대조 | 대조 기준이 모호 (`clientOrderId` 없음) | 같음 | 단독으로는 약함 |
| C. 10분 안에는 A, 그 뒤에는 B + 웹소켓 이벤트 대조, 확정 전에는 같은 종목 주문 막기 | 확정됨 | 대부분 확정, 아니면 사람에게 넘김 | **추천** (ORD-2와 같음. 대조 키만 보강) |

틀렸다고 판명될 조건: 오류로 끝난 첫 요청을 같은 키로 다시 보낼 때 토스가 새로 처리한다면, A 단계에서 "실패 후 재시도"도 안전해진다. 반대로 성공 결과가 10분보다 짧게 캐시되는 사례가 보이면 A 단계 시간을 줄인다.

### 3.3 가정용 유동 IP

Q3 표 참고. 추천은 (가) 현재 IP 등록 + REL-1 안내이고, IP가 한 달에 여러 번 바뀌면 (나) 고정 IP 출구로 옮긴다. 이유: 개인 1명, 배포 없음, 주문은 사람 승인 뒤에만 나가므로 동기화가 잠시 멈춰도 손실이 크지 않다. 틀렸다고 판명될 조건: 실제 회선의 IP 변경 빈도가 높아 아침 브리핑·장중 동기화가 자주 멈출 때.

### 3.4 스펙 변경 감지

| 방식 | 같은 번호의 내용 변경 감지 | 비용 | 판단 |
|---|---|---|---|
| `info.version`만 비교 | 못 함 (1.2.28에서 거래내역 API가 번호 변경 없이 추가됨) | 낮음 | 부족 |
| 정규화 JSON 해시 + `versions.json` + 생성 타입 차이 검사 | 함 | 낮음 | **추천** |

## 4. 위험과 미확인 항목

**위험**

| 위험 | 영향 | 대응 |
|---|---|---|
| 키에 권한 범위가 없다 | 키가 새면 주문까지 된다 | 키는 코어·키체인에만. AI 프로세스는 네트워크·파일 도구 없음 (SEC-1, SEC-2). 주문 HTTP 경로를 `order-execution` 전용 타입으로 |
| 토큰이 클라이언트당 1개 | 다른 도구·기기가 같은 키로 토큰을 받으면 앱의 REST·웹소켓이 끊긴다. 둘이 서로 재발급하면 무한 반복 | 재발급 1회 제한, `toss-smoke`는 앱이 켜져 있으면 거부 (T-12). 사용자에게 "이 키는 이 앱 전용"이라고 안내 |
| 웹소켓 계정당 2연결 | 다른 도구가 연결하면 앱 연결이 밀려난다 | 앱은 1연결만 쓴다. close code 없는 끊김을 연결 수 초과 가능성으로 표시 |
| 스펙이 같은 버전 번호로 바뀐다 | 생성 타입이 실제 응답과 어긋남 | 해시 감시, 응답 스키마 검증 실패를 "버그" 분류로 보고 (architecture.md 3장) |
| 응답 enum 값이 추가·삭제된다 | 빠짐없는 match가 깨지거나 역직렬화 실패 | 모든 외부 enum에 `Unknown(String)` 변형을 둔다 |
| 국내 취소로 저장한 거래가 사라진다 | 손익·평균가 계산 오류 | 3.1 방식 C |
| 거래내역·주문·이벤트에 서로 잇는 ID가 없다 | 체결과 거래내역, 제안과 거래내역 연결이 추정이 된다 | 성적표(R-12)는 주문 상세(`orderId`, 평균 체결가)를 기준으로 하고 거래내역은 현금 흐름에만 쓴다 |
| 대량 주문·비정상 조회 제재 | 매매 제한, 1개월 중지, 해지 | 사람 승인 구조라 주문 수는 적다. 조회는 그룹 한도의 60% (T-03) |
| 공식 모의 환경 없음 | 주문 경로의 실전 확인이 실거래뿐 | 가짜 서버 계약 테스트 + O-15 소액 실주문 |
| 시세를 LLM에 보내는 것이 약관 제5조 ③ "제3자 제공"에 해당할 여지 | 약관 위반 해석 가능성 | R-13에서 판단. 토스 자신은 LLM 연동을 홍보하고 이용 안내에서 외부 전송 책임만 사용자에게 둔다 |

**미확인 (실측이나 토스 문의가 필요)**

1. 허용 IP 최대 개수, CIDR 대역 등록 가능 여부, 변경 반영 시간.
2. 실제 `expires_in` 값 (스펙 예시 86400).
3. 사용자당 키(클라이언트)를 여러 개 만들 수 있는지.
4. 403 `forbidden`이 나는 조건, `order-limit-exceeded`의 "주문 설정 한도"를 어디서 설정하는지.
5. 거래내역의 실제 조회 가능 과거 기간. 오래된 결제 완료 행이 바뀌는지.
6. 분할·병합·종목 코드 변경이 거래내역에 어떻게 나오는지. 과거 행의 `symbol`이 당시 코드인지 현재 코드인지.
7. 보유 주식의 `cost`가 매수 비용인지 예상 매도 비용인지. 토스 앱 화면이 `amount`와 `amountAfterCost` 중 무엇과 같은지. 손익률 환산 환율 종류와 시각.
8. `cashBuyingPower`에 미체결 매수·매도 미결제 대금·환전이 어떻게 반영되는지. 토스 앱의 예수금과의 차이.
9. 1억 원 판정의 환율과 금액 계산식. 1억 원 미만에 `confirmHighValueOrder: true`를 보낼 때의 동작.
10. 오류로 끝난 주문 요청을 같은 `clientOrderId`로 다시 보낼 때의 동작.
11. 해외 주식 거래 신청이 따로 필요한지와 그때의 오류 코드. 투자자지시 거래소 설정 메뉴 위치.
12. 공식 변경 공지 채널.
13. 국내 `OPG` 지원 여부 (REST 스펙과 AsyncAPI가 다르다).

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·항목 | 지금 | 바꿀 내용 |
|---|---|---|
| design.md 1장 "토스증권 Open API v1.2.28" | 버전 번호로 고정 | 버전 번호와 함께 파일 해시(sha256 `c5918961…036a`)로 고정한다. 같은 번호로 내용이 바뀐다는 사실을 적는다 |
| design.md 6장 규칙 "같은 종목의 반대 방향 미체결 주문" 검증 | 시장 구분 없음 | 국내 주식 일반 주문에만 적용한다 (토스 규칙과 같게). 미국 주식에는 막지 않는다. 막고 싶으면 원칙으로 따로 둔다 |
| design.md 6장 1억 원 규칙, tasks.md O-09 | 1억 원 이상이면 확인 | 판정 환율이 미확인이므로 `max(rate, midRate)` 환산액이 1억 원의 95% 이상이면 확인을 받는다. 판정식을 코드 상수 하나로 둔다 |
| design.md 6장 / 설정 | 언급 없음 | 미국 주식 매수는 달러 `cashBuyingPower`만 본다. 자동 환전·API 환전이 없다는 사실을 제안 카드와 게이트 검증에 넣는다 |
| design.md 7.2 ORD-2 | 10분 뒤 "주문 조회와 주문 이벤트로 대조" | 조회·이벤트에 `clientOrderId`가 없으므로 대조 키를 (종목, 방향, 호가 유형, 가격, 수량·금액, 전송 시각 이후 `orderedAt`)로 정한다. 점검(`maintenance`, 예시 600초)은 10분 창을 다 쓸 수 있으므로 점검 응답 뒤에는 바로 대조 단계로 간다 |
| design.md 4장 `get_portfolio` 설명, 7.3 ACC-6 | "토스 API는 예수금을 주지 않는다" | 현금 자리에 통화별 `cashBuyingPower`를 "매수가능금액"으로 쓰고, 총자산은 근사라고 표시한다. ACC-6 대조 항목에 `amount`/`amountAfterCost` 중 어느 것이 앱과 같은지, 손익률 환율을 추가한다 |
| design.md 위험표 "토스 키에 읽기 전용 권한이 없음" | "문서에서 찾지 못했다" | "스펙의 OAuth scopes가 비어 있다(확인됨)"로 바꾼다. 서버 쪽 차단 수단은 IP 제거·키 삭제뿐이고 조회까지 막는다는 점을 적는다 |
| design.md 위험표 (추가) | 없음 | "같은 키를 다른 도구에서 쓰면 토큰·웹소켓이 서로 끊긴다", "공식 모의 환경 없음", "국내 취소로 거래 행이 사라짐" |
| architecture.md 3장 오류 분류 | 세 종류 | 위 오류표의 분류를 그대로 쓴다. `token-revoked` 반복은 "사용자가 고칠 수 있음(키 중복 사용)"으로 분류한다. 오류 파서는 `requestId`와 `referenceId`를 둘 다 받는다 |
| architecture.md 5장 저장소 "거래내역: 원본 보존" | 원본 보존 | 결제 완료 행을 멀티셋으로 저장하고, 사라진 행은 삭제 표시를 남긴다 (3.1 방식 C) |
| architecture.md 2장 crate 배치 | `broker-toss`가 `BrokerRead`와 `BrokerTrade`를 함께 구현 | HTTP 클라이언트를 읽기용과 주문용으로 나누고, 주문 경로 요청을 만드는 타입은 `order-execution` 승인 증거 없이는 만들 수 없게 한다 |
| tasks.md A-07 | "결제 내역은 R-01에서 정한 기준으로 중복 없이 추가" | "결제일 기준 최근 14일 창을 다시 받아 멀티셋으로 맞춤. 줄어든 행은 삭제 표시. 하루 1회 전체 기간 검사" |
| tasks.md T-01 | 스펙 차이가 생성물에 드러남 | 공식 `versions.json`, 파일 `Last-Modified`, 정규화 해시를 감시하는 검사를 추가한다. 외부 enum은 `Unknown(String)` 변형을 갖는다. R-09의 와일드카드 match 금지와 함께 쓰려면 `Unknown`을 명시 변형으로 둬야 한다 |
| tasks.md T-02 | 단일 토큰 | `token-revoked`는 재발급 1회 후 중단. 만료 전 계획 재발급 순서: 발급 → 웹소켓 닫기 → 재연결 → 재선언 → 주문 재조회 |
| tasks.md T-16 | 주문 전송·정정·취소 | 정정·취소는 새 `orderId`를 돌려주므로 원주문 → 새 주문 연결을 앱이 저장한다. KR 정정은 수량 필수, US 정정은 가격만 |
| tasks.md T-17 | 재연결, 2연결 | 주문 이벤트는 "다시 조회하라는 신호"로만 쓴다 (순번·ID 없음). 수신 루프와 처리를 분리한다 (2초 막히면 서버가 끊음). 이전 연결을 닫고 재연결한다 |
| tasks.md T-00 | 허용 IP 등록, 응답 형태 확인 | 미확인 1–3, 5–8을 실측 항목으로 추가한다. 투자자지시 거래소 SOR 설정, 상품별 사전 요건, 달러 환전을 사용자 준비 항목으로 추가한다 |
| tasks.md O-15 | 소액 실주문 | 미확인 9–10(1억 미만에 확인 플래그, 오류 후 같은 키 재전송)을 소액으로 확인할 수 있는 범위에서 확인 항목으로 추가한다 |
| research.md R-13 | 질문 3 | 약관 제5조 ③의 "제3자 제공"과 LLM 전송의 관계를 질문에 넣는다 |

## 6. 엔드포인트 표

스펙 1.2.28에서 기계적으로 뽑았다. "계좌 헤더"는 `X-Tossinvest-Account` 필수 여부다. 금액 필드는 모든 엔드포인트에서 문자열 십진수(`type: string, format: decimal`, 대부분 `maxLength: 30`)이고 `number` 형식의 금액은 없다. null이 될 수 있는 금액은 `["string","null"]`이다. 비율(손익률, 수수료율)도 같은 형식의 소수 비율이다 (0.1516 = 15.16%).

| 메서드 | 경로 | 속도 제한 그룹 (초당) | 계좌 헤더 | 페이징 | 금액 형식·비고 |
|---|---|---|---|---|---|
| POST | /oauth2/token | AUTH (5) | 아니오 | 없음 | 금액 없음. form-urlencoded, OAuth2 오류 형식 |
| GET | /api/v1/orderbook | MARKET_DATA (15) | 아니오 | 없음 | 문자열 십진수 |
| GET | /api/v1/prices | MARKET_DATA (15) | 아니오 | 없음 (symbols 다건) | 문자열 십진수 |
| GET | /api/v1/trades | MARKET_DATA (15) | 아니오 | `count` (최대 50, 과거 방향 없음) | 문자열 십진수 |
| GET | /api/v1/price-limits | MARKET_DATA (15) | 아니오 | 없음 | 문자열 십진수 |
| GET | /api/v1/candles | MARKET_DATA_CHART (20) | 아니오 | `count` (최대 200) + `before` | 문자열 십진수, `adjusted` |
| GET | /api/v1/stocks | STOCK (5) | 아니오 | 없음 (symbols 최대 200) | 문자열 십진수 (발행주식수 등) |
| GET | /api/v1/stocks/all | STOCK_ALL (1) | 아니오 | 없음 (전체) | 금액 없음 |
| GET | /api/v1/stocks/{symbol}/warnings | STOCK (5) | 아니오 | 없음 | 금액 없음 |
| GET | /api/v1/stocks/{symbol}/investor-trading | STOCK_TRADING_TREND (10) | 아니오 | `count` + `until` | 문자열 십진수 |
| GET | /api/v1/stocks/{symbol}/program-trades | STOCK_TRADING_TREND (10) | 아니오 | `count` + `until` | 문자열 십진수 |
| GET | /api/v1/stocks/{symbol}/short-selling | STOCK_TRADING_TREND (10) | 아니오 | `count` + `until` | 문자열 십진수 |
| GET | /api/v1/stocks/{symbol}/credit-trades | STOCK_TRADING_TREND (10) | 아니오 | `count` + `until` | 문자열 십진수 |
| GET | /api/v1/stocks/{symbol}/securities-lending | STOCK_TRADING_TREND (10) | 아니오 | `count` + `until` | 문자열 십진수 |
| GET | /api/v1/exchange-rate | MARKET_INFO (3) | 아니오 | 없음 | 문자열 십진수 (`rate`, `midRate`, `basisPoint`) |
| GET | /api/v1/market-calendar/KR | MARKET_INFO (3) | 아니오 | 없음 (`date`) | 금액 없음 |
| GET | /api/v1/market-calendar/US | MARKET_INFO (3) | 아니오 | 없음 (`date`) | 금액 없음 |
| GET | /api/v1/rankings | RANKING (5) | 아니오 | `count` | 문자열 십진수 |
| GET | /api/v1/rankings/investor-trading | RANKING (5) | 아니오 | `count` | 문자열 십진수 (원화) |
| GET | /api/v1/market-indicators/prices | MARKET_INDICATOR (10)* | 아니오 | 없음 | 문자열 십진수 |
| GET | /api/v1/market-indicators/{symbol}/candles | MARKET_INDICATOR_CHART (5) | 아니오 | `count` + `before` | 문자열 십진수 |
| GET | /api/v1/market-indicators/{symbol}/investor-trading | MARKET_INDICATOR (10) | 아니오 | `count` + `until` | 문자열 십진수 |
| GET | /api/v1/sectors | SECTOR (5) | 아니오 | 없음 | 금액 없음 |
| GET | /api/v1/sectors/rankings | SECTOR_RANKING (5) | 아니오 | 없음 | 문자열 십진수 (US도 원화 환산) |
| GET | /api/v1/sectors/{sectorId} | SECTOR (5) | 아니오 | 없음 | 문자열 십진수 |
| GET | /api/v1/sectors/{sectorId}/stocks | SECTOR (5) | 아니오 | 없음 (전체) | 문자열 십진수 (US도 원화 환산) |
| GET | /api/v1/sectors/{sectorId}/etfs | SECTOR (5) | 아니오 | 없음 | 금액 없음 |
| GET | /api/v1/accounts | ACCOUNT (1) | 아니오 | 없음 | 금액 없음. `accountSeq`는 정수 |
| GET | /api/v1/holdings | ASSET (5) | **예** | 없음 (`symbol` 필터) | 문자열 십진수. 요약은 `{krw, usd\|null}` |
| GET | /api/v1/transactions | TRANSACTION (5) | **예** | `cursor` + `limit`(≤100) + `hasNext`, 날짜는 결제일 기준 `from`/`to` | 문자열 십진수, 부호 없는 절대값 |
| GET | /api/v1/orders | ORDER_HISTORY (5) | **예** | OPEN: 전량(커서 무시). CLOSED: `cursor` + `limit`(≤100). `from`/`to`는 주문일 | 문자열 십진수 (거래 통화) |
| POST | /api/v1/orders | ORDER (10, 09:00–09:10 KST 10) | **예** | 해당 없음 | 요청 `price`·`quantity`·`orderAmount`는 문자열 십진수 (`^\d+(\.\d+)?$`) |
| GET | /api/v1/orders/{orderId} | ORDER_HISTORY (5) | **예** | 해당 없음 | 문자열 십진수 |
| POST | /api/v1/orders/{orderId}/modify | ORDER (10) | **예** | 해당 없음 | 요청 문자열 십진수. KR 수량은 `^\d+$` |
| POST | /api/v1/orders/{orderId}/cancel | ORDER (10) | **예** | 해당 없음 | 금액 없음 |
| POST | /api/v1/conditional-orders | CONDITIONAL_ORDER (5) | **예** | 해당 없음 | 문자열 십진수 |
| GET | /api/v1/conditional-orders | CONDITIONAL_ORDER_HISTORY (10) | **예** | `cursor` + `limit` (`status`) | 문자열 십진수 |
| GET | /api/v1/conditional-orders/{conditionalOrderId} | CONDITIONAL_ORDER_HISTORY (10) | **예** | 해당 없음 | 문자열 십진수 |
| DELETE | /api/v1/conditional-orders/{conditionalOrderId} | CONDITIONAL_ORDER (5) | **예** | 해당 없음 | 금액 없음 |
| POST | /api/v1/conditional-orders/{conditionalOrderId}/modify | CONDITIONAL_ORDER (5) | **예** | 해당 없음 | 문자열 십진수. 수정하면 새 ID |
| GET | /api/v1/buying-power | ORDER_INFO (6, 09:00–09:10 KST 3) | **예** | 해당 없음 | 문자열 십진수 (KRW 정수, USD 소수) |
| GET | /api/v1/sellable-quantity | ORDER_INFO (6, 피크 3) | **예** | 해당 없음 | 문자열 십진수 (US 소수 수량 가능) |
| GET | /api/v1/commissions | ORDER_INFO (6, 피크 3) | **예** | 해당 없음 | 문자열 십진수 (수수료율 소수 비율) |
| WS | wss://openapi-ws.tossinvest.com/ws/v1 | 선언 5회/초, 연결 2개/계정, 구독 100건/연결 | 주문 채널은 `codes`에 `accountSeq` 문자열 | 해당 없음 | 문자열 십진수 |

\* 스펙의 `market-indicators/prices` 설명은 그룹을 `MARKET_INDICATOR`로 적는다. overview 한도표에는 `MARKET_INDICATOR_PRICE`(10)라는 그룹이 따로 있지만 그 그룹을 쓰는 엔드포인트는 스펙에 없다. 한도 값이 같아 영향은 없지만 문서 불일치다.

한도는 "운영 상황에 따라 사전 공지 없이 조정"되며 실제 값은 응답 머리말 `X-RateLimit-Limit`(버킷 크기), `X-RateLimit-Remaining`, `X-RateLimit-Reset`(토큰 1개 재충전까지 초), 429일 때 `Retry-After`로 확인한다. 성공 응답에도 항상 붙는다 [S4, S3].

## 7. 직접 실행한 것

실험 파일 위치: `<세션 임시 폴더>/research/R-01/`

| 실험 | 결과 |
|---|---|
| 공식 `latest/openapi.json`, `asyncapi.json`, `overview.md`, `faq.md`, `versions.json`, `v1.0`·`v1.1`·`v1.2` 스펙 내려받기 | 저장소 사본과 최신본이 바이트 단위로 같음. 모든 파일 `Last-Modified: Wed, 04 Nov 2026 01:48:38 GMT` |
| 약관·소개 페이지를 헤드리스 Chromium(Playwright)으로 렌더링 | 약관 본문은 `docs-api.tossinvest.com/api/v2/contents/752`에서 불러온다. 소개 페이지의 접힌 "API 이용 안내"를 펼쳐 원문 확보 |
| 스펙 기계 분석 (`slim.py`, `endpoints.tsv`, 오류 코드 수집) | 6장 표와 Q7 오류 목록 |
| 스냅샷 22개 비교 (`snaps/`, `breaking.py`) | Q9 표. 요청 쪽 하위 호환 파괴 없음, 응답 enum 삭제 1건, 응답 필수 필드 추가 1건, 버전 번호를 올리지 않은 엔드포인트 추가 2건 (1.2.13 `stocks/all`, 1.2.28 `transactions`) |
| 키 없이 `GET /api/v1/prices` 1회 | 401 `edge-blocked`, 본문에 `referenceId` (Q7 끝) |

실제 키가 필요한 확인(토큰 수명, 허용 IP 동작, 보유 금액 대조, 주문)은 하지 않았다. 4장 미확인 목록이 T-00·T-13·O-15에서 확인할 항목이다.

## 8. 출처 목록

모두 2026-11-11 확인.

- [S1] 토스증권, 「오픈 API 서비스 이용 약관」 (제정 2026-05-18, 개정 2026-08-12): https://corp.tossinvest.com/ko/terms/v2?id=752
- [S2] 토스증권 Open API 소개 페이지 (FAQ, "API 이용 안내", 준법감시인 심사필 제2026-146호): https://corp.tossinvest.com/ko/open-api
- [S3] 토스증권 Open API FAQ (Markdown): https://openapi.tossinvest.com/openapi-docs/faq.md · 브라우저판 https://developers.tossinvest.com/docs/faq
- [S4] 토스증권 Open API overview (Rate Limits, 오류, 웹소켓 연동): https://openapi.tossinvest.com/openapi-docs/overview.md · 브라우저판 https://developers.tossinvest.com/docs
- [S5] 토스증권 Open API OpenAPI JSON 1.2.28: https://openapi.tossinvest.com/openapi-docs/latest/openapi.json (Markdown판 https://openapi.tossinvest.com/openapi-docs/latest/api-reference/README.md)
- [S6] 토스증권 웹소켓 AsyncAPI JSON 1.2.3: https://openapi.tossinvest.com/openapi-docs/latest/asyncapi.json
- [S7] 공식 버전 목록: https://openapi.tossinvest.com/openapi-docs/versions.json · 보관본 https://openapi.tossinvest.com/openapi-docs/v1.0/openapi.json , https://openapi.tossinvest.com/openapi-docs/v1.1/openapi.json , https://openapi.tossinvest.com/openapi-docs/v1.2/openapi.json
- [S9] 개발자 문서 안내 `llms.txt`: https://developers.tossinvest.com/llms.txt
- [S10] 토스증권 보도자료 「토스증권, '오픈API' 서비스 정식 출시」 (2026-08-26): https://corp.tossinvest.com/ko/news-room/detail?id=52615
- [S11] (2차, 비공식) tossinvest-cli 저장소. CHANGELOG https://github.com/JungHoonGhae/tossinvest-cli/blob/main/CHANGELOG.md , 공식 스펙 스냅샷 이력 https://github.com/JungHoonGhae/tossinvest-cli/commits/main/docs/migration/openapi.latest.json (조사 시점 커밋 `a8c77d53`)
- [S12] (2차, 블로그) AlgoLab, 「토스증권 오픈API 자동매매 — 발급·주문·호출한도 정리」 (2026-08-04 발행, 일부 2026-10-14 재확인): https://algolab.co.kr/blog/toss-securities-open-api-guide-2026
- [S13] (2차, 블로그) 「토스증권 Open API로 AI 트레이딩」 (2026-07-25): https://brunch.co.kr/@9eec7057ec5d4d4/103
- [S14] (2차, 비공식) tossinvest-cli PR #388 (Open API 키 연결 도우미, 2026-11-07 병합): https://github.com/JungHoonGhae/tossinvest-cli/pull/388
