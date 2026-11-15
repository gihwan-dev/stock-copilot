# ADR 0088: T-18 TossEndpoint와 HTTP 클라이언트

- 상태: 확정 (2026-11-16)

- 최초 기준: ecb6e5a73b3fe5a394d55c0803bb203ad984cdf6

## 결정

메인 후속 결정에 따라 작업 절이 승인한 reqwest 및 TLS/런타임 전이 패키지는 일반 새 외부 패키지 금지의 예외다. reqwest =0.13.5는 기존 잠금 버전을 직접 의존성으로 사용한다. default-features=false, features=[rustls]다. HTTP/1만 사용하고 JSON·쿠키·압축·시스템 프록시 기능은 켜지 않는다. tokio =1.54.1은 기존 잠금 버전이며 기본 기능 없이 rt/time만 직접 켠다. 작업 공간의 Tauri 기능 통합으로 나타나는 추가 전이 경로와 직접 켠 기능을 구분한다.

reqwest의 설치된 매니페스트에서 rustls 기능은 rustls-platform-verifier와 기본 AWS-LC 제공자를 선택한다. macOS에서는 플랫폼 신뢰 저장소의 인증서 검증을 사용한다. reqwest가 제공하는 기본 권장 제공자 구성을 그대로 사용해 별도의 직접 TLS/소켓 API와 전역 제공자 설치를 추가하지 않는다. rustls-no-provider는 별도 제공자 설치와 직접 TLS 의존을 요구하므로 선택하지 않는다. OpenSSL과 native-tls는 링크하지 않는다. 잠금의 ring/quinn 및 다른 운영체제 패키지는 Cargo의 전체 대상/선택적 의존 해석에 포함될 수 있으므로 잠금 존재와 macOS 활성 제품 그래프를 구분한다.

TossEndpoint의 필드는 비공개이며 외부 생성 수단은 production()과 local(port)뿐이다. 제품 생성자는 인자 없이 고정 주소 상수 하나를 쓴다. local은 0 포트를 거부하고 1~65535에서 127.0.0.1만 만든다. 임의 URL·호스트·IP 문자열 생성자는 없다. 루프백 테스트 resolver는 localhost만 127.0.0.1로 해석하고 다른 이름은 로컬 오류로 거부한다. 숫자 IP는 DNS를 우회하므로 생성자 자체의 루프백 고정이 첫 방어다.

Rust는 다른 crate의 특정 파일만 허용하는 가시성을 제공하지 않는다. feature는 하네스가 금지한다. 메인은 공개 production 생성자를 승인하고 호출 위치를 구조 검사로 강제하도록 계약을 변경했다. 컴파일 차단이라고 주장하지 않는다. 조립 파일은 apps/desktop/src-tauri/src/main.rs 하나이며 호출 없이 함수 값으로 참조하거나 use 별칭/타입 별칭/원시 식별자/매크로에서 사용하는 경우도 검사한다. 보수적으로 production 식별자는 제품 생성용 이름으로 예약한다. 호스트 상수는 endpoint.rs의 PRODUCTION_ORIGIN만 허용한다. 이전 조립 루트와 toss-smoke 호스트 허용은 제거했다.

| 실제 컴파일 실험 | 결과 |
|---|---|
| 라이브러리 pub(crate) production을 별도 composition_root에서 호출 | E0624, 컴파일 실패 |
| 같은 타입의 public local을 외부 consumer에서 호출 | 컴파일 성공 |
| production을 pub으로 바꾼 뒤 composition_root에서 호출 | 컴파일 성공 |
| 동일 public production을 일반 외부 consumer에서 호출 | 컴파일 성공 |

실험은 target/T-18/visibility-yfe663fe에 있다. 제품 소스의 문자열 우회로가 아니라 계약 선택의 근거다.

| 구성 | 제품 | 테스트 |
|---|---|---|
| 주소 | 고정 HTTPS 상수 | 127.0.0.1과 주입 포트 |
| HTTPS 전용 | 켬 | 루프백 평문 허용 |
| resolver | 시스템 | localhost만 루프백 반환 |
| 리다이렉트 | 금지 | 금지 |
| 시스템/환경 프록시 | no_proxy, 관련 기능 끔 | 동일 |
| 연결/전체 제한 | 5초/30초 | 동일 |
| 인증서 검증 | rustls 플랫폼 검증, 끄기 없음 | 평문 루프백만 사용 |
| HTTP 버전 | HTTP/1 | HTTP/1 |

타임아웃 수치는 기존 문서에 없어 보수적인 연결 5초·전체 30초로 정한다. 원시 본문은 8MiB 상한을 두고 초과하면 본문 없는 닫힌 오류를 반환한다. 리다이렉트는 따라가지 않고 원시 3xx를 반환한다.

## 공개 API와 경계

요청 수단은 TossHttpClient::get(ReadRequest, SecretValue)와 issue_token(client, client_id, client_secret)뿐이다. ReadRoute는 닫힌 GET 경로 목록, RouteId는 단일 경로 조각 검증, QueryKey는 스펙의 닫힌 쿼리 이름이다. 계좌 헤더도 SecretValue를 받는다. 임의 메서드·임의 경로·임의 헤더 및 내부 reqwest 클라이언트는 외부에서 접근하지 못한다. 토큰 발급은 고정 /oauth2/token의 client_credentials form 요청이다.

RawResponse는 상태·Retry-After·세 가지 rate-limit 헤더·본문 바이트를 보존한다. read_error/token_error는 기존 T-04 매퍼 서명을 연결한다. 토큰·계좌 헤더는 sensitive이며 transport 오류는 원본 reqwest 오류를 보관하지 않는다. 요청·응답의 Debug는 고정 문자열이다. 키와 토큰을 URL에 붙이지 않는다. 실제 토스 요청과 실제 자격 증명은 사용하지 않는다.

## 의존성 등록

아래는 최초 기준 잠금과 비교해 추가된 이름/버전 전체다. 정확한 전이 버전은 Cargo.lock으로 고정한다. 기존 잠금에 있었지만 macOS 제품에서 새로 활성화된 reqwest/tokio 전이 패키지도 deny 허용 목록에 등록했다. 출처는 기존 crates.io 한 곳뿐이다. AWS-LC와 untrusted 배포본의 실행 가능한 보조 파일은 개별 경로와 SHA-256으로만 등록하며 파일군 전체 예외는 두지 않는다. 등록이 해당 스크립트를 제품에서 실행한다는 뜻은 아니다.

| 이름 | 버전 | 라이선스 |
|---|---|---|
| aws-lc-rs | 1.19.0 | ISC AND (Apache-2.0 OR ISC) |
| aws-lc-sys | 0.47.0 | ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) |
| cfg_aliases | 0.2.2 | MIT |
| cmake | 0.1.58 | MIT OR Apache-2.0 |
| fs_extra | 1.3.0 | MIT |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| hyper-rustls | 0.27.9 | Apache-2.0 OR ISC OR MIT |
| jni | 0.22.4 | MIT OR Apache-2.0 |
| jni-macros | 0.22.4 | MIT OR Apache-2.0 |
| jobserver | 0.1.35 | MIT OR Apache-2.0 |
| lru-slab | 0.1.2 | MIT OR Apache-2.0 OR Zlib |
| openssl-probe | 0.2.2 | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| quinn | 0.11.9 | MIT OR Apache-2.0 |
| quinn-proto | 0.11.14 | MIT OR Apache-2.0 |
| quinn-udp | 0.5.14 | MIT OR Apache-2.0 |
| rand | 0.9.5 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT |
| rustls-native-certs | 0.8.3 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.16.0 | MIT OR Apache-2.0 |
| rustls-platform-verifier | 0.7.0 | MIT OR Apache-2.0 |
| rustls-platform-verifier-android | 0.1.1 | MIT OR Apache-2.0 |
| rustls-webpki | 0.103.16 | ISC |
| schannel | 0.1.30 | MIT |
| security-framework | 3.7.0 | MIT OR Apache-2.0 |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 |
| subtle | 2.6.1 | BSD-3-Clause |
| tokio-rustls | 0.26.4 | MIT OR Apache-2.0 |
| untrusted | 0.9.0 | ISC |
| webpki-root-certs | 1.0.12 | CDLA-Permissive-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.53.1 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.53.1 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.53.1 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.53.1 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.53.1 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.53.1 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.53.1 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |

## 보호 파일과 기존 고정 자료

- deny.toml: 승인된 제품 HTTPS 의존성·빌드 스크립트·개별 배포 파일 해시 등록.
- harness/arch-check/rules.toml: 호스트 위치를 단일 상수 파일로 변경. HTTP/TLS는 reqwest를 통하는 경로만 허용하며, 플랫폼 인증서 검증의 security-framework 및 작업 공간 기능 통합의 절차 매크로/런타임 소켓 경로를 한정 등록.
- 기존 tossinvest-host-in-main-and-toss-smoke-pass 고정 자료를 toss-endpoint-origin-and-root-factory-pass로 이름을 맞춤: 실제 호스트 문자열을 없애고 main의 production 참조와 toss-smoke의 local 호출로 변경. 새 계약의 단일 상수 파일을 같은 고정 자료에 추가. 기존 테스트 단언을 느슨하게 하지 않는다.
- .codex/config.toml, harness/sandbox/lock-fragment.json, harness/claude-hooks/hashes.json: 저장소 공식 생성기로만 갱신.

## 인계

- T-02: 토큰 응답 파싱, 저장, 만료, 단일 발급, 재발급 순서를 구현한다. 토큰 발급 전송 함수는 그 절차를 실행하지 않는다.
- T-03: 원시 제한 헤더의 검증·속도 제한·재시도 정책을 구현한다.
- T-05/T-07/T-08/T-09/T-10/T-11/T-15: 포트 입력을 닫힌 경로·쿼리·헤더에 연결하고 응답 파싱 및 요청 사실 결합을 구현한다. 전송 성공이 응답 의미 검증은 아니다.
- T-20: 녹화 계약 테스트 틀을 확장한다. 이번 가짜 서버는 루프백의 원시 전송 검증 도우미다.

## 검증 기록

초기 공개 API 미정의 E0432로 회귀 red를 확인했다. 프로젝트 설정 hooks=true는 확인했으나 현재 세션의 프로젝트/훅 신뢰 적용은 확인할 수 없다. 시작 frozen install과 lefthook install은 성공했다. 홈 설정·인증 파일은 읽거나 고치지 않았다. 제품 검사와 리뷰/변이의 최종 결과는 이어서 기록한다.

## 손 변형 결과

mktemp -d로 만든 복사본에서 최초 기준 실행 및 복구 실행을 통과했다. 아래 28개는 모두 실제 컴파일 후 지정 테스트 실패로 검출했다. 생존 0, 컴파일 불가 0이다. 원시 결과는 target/T-18/manual-results.json과 manual-logs에 있다. 표의 문자열은 제품의 닫힌 분기 선택을 바꾸는 변형이다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|

| 1 | Accounts GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 2 | Holdings GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 3 | Transactions GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 4 | Quotes GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 5 | Candles GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 6 | BuyingPower GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 7 | SellableQuantity GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 8 | Orders GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 9 | FxRate GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 10 | Orderbook GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 11 | PriceLimits GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 12 | CommissionRate GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 13 | Instruments GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 14 | Stocks GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 15 | Rankings GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 16 | Sectors GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 17 | SectorRankings GET 경로 교환 | request::tests::closed_routes_match_the_openapi_get_paths | 검출 |
| 18 | Symbol 쿼리 키 교환 | request::tests::all_query_keys_are_closed_and_exact | 검출 |
| 19 | Symbols 쿼리 키 교환 | request::tests::all_query_keys_are_closed_and_exact | 검출 |
| 20 | Cursor 쿼리 키 교환 | request::tests::all_query_keys_are_closed_and_exact | 검출 |
| 21 | Limit 쿼리 키 교환 | request::tests::all_query_keys_are_closed_and_exact | 검출 |
| 22 | Status 쿼리 키 교환 | request::tests::all_query_keys_are_closed_and_exact | 검출 |
| 23 | Market 쿼리 키 교환 | request::tests::all_query_keys_are_closed_and_exact | 검출 |
| 24 | Currency 쿼리 키 교환 | request::tests::all_query_keys_are_closed_and_exact | 검출 |
| 25 | ID 상한 128 포함 거부 | request::tests::dynamic_identifiers_cannot_escape_their_single_path_segment | 검출 |
| 26 | 점 경로 조각 허용 | request::tests::dynamic_identifiers_cannot_escape_their_single_path_segment | 검출 |
| 27 | 상위 경로 조각 허용 | request::tests::dynamic_identifiers_cannot_escape_their_single_path_segment | 검출 |
| 28 | 포트 0 허용 | tests::zero_port_and_invalid_secret_are_closed_errors | 검출 |

## 자체 리뷰 보완과 최초 정상 훅

호스트 검사에 매크로 토큰의 이스케이프 리터럴·C 문자열과 endpoint.rs의 중첩 동명 상수 우회가 남아 있음을 자체 리뷰에서 확인했다. 회귀 테스트를 먼저 실행해 1실패/2통과를 확인한 뒤 매크로 문자열 해석과 최상위 비공개 단일 문자열 상수만 허용하도록 보완했다. compiler visibility를 추가로 주장하지 않는다.

최초 정상 커밋 훅은 fresh-build·commit-guard를 통과하고 전체 check로 승격했다. 프런트 검사와 Rust 빌드 뒤 cargo-deny가 추가 배포 실행 파일 두 개(proptest의 테스트 보조 스크립트, tracing의 매크로 테스트 보조 스크립트)를 지적해 실패했다. 이 실패를 숨기거나 훅을 끄지 않았다. 개별 경로·SHA-256 등록으로 고쳤고 전체 정상 훅을 다시 실행한다. 별도 조립 루트 좁은 테스트는 최초에 frontendDist 부재로 컴파일이 실패했으며 정상 전체 검사의 프런트 빌드 뒤 결과와 구분한다.

macOS 활성 normal/build 의존 그래프에서 rustls 0.23.46, rustls-platform-verifier 0.7.0, security-framework 3.7.0, AWS-LC 1.19.0/0.47.0을 확인했다. OpenSSL, native-tls, ring, quinn은 이 활성 그래프에 없다. 잠금 전체 목록과 실제 macOS 활성 그래프를 구분한다.

## 리뷰 1 수정 결정

독립 Opus 지적 11건을 새 Codex가 비판했다. 추가 생성 경로 검출 누락과 토큰 응답의 일반 바이트 소유를 차단으로 수용했다. include 단독 검사 누락은 전체 source-check가 이미 막으므로 전체 하네스 우회로 해석하지 않는다. 나머지 검증 누락과 문서 사유를 보완한다. 판정표는 docs/reviews/T-18.md에 있다.

endpoint의 필드는 같은 crate의 다른 모듈에서도 조립할 수 없게 닫았다. 지정 상수와 Production 변형의 참조는 endpoint.rs의 정식 inherent production 생성자 안으로 한정한다. Default·추가 생성자·상수 반환 함수를 음성 자료로 고정한다. 이 가시성은 조립 파일에서의 생성자 참조 제한을 대체하지 않고 보완한다.

원시 본문은 읽기/토큰 구분 없이 SecretValue로 보관한다. 수신 중 각 소유 조각도 SecretValue이며 오류나 크기 초과로 중단돼도 소유 조각은 삭제된다. body()는 SecretValue 참조를 돌려주므로 바이트 파싱은 as_exposed_secret()를 명시해야 한다. T-02의 JSON 파싱·토큰 만료 관리는 구현하지 않는다. HTTP SDK 내부의 버퍼 복사까지 지울 수 있다고 주장하지 않는다.

HTTPS 회귀는 실제 열린 루프백 수신기에 연결이 오지 않아야 통과한다. 타임아웃은 실제 빌드에 사용하는 reqwest ClientBuilder의 설정을 설치된 고정 버전의 Debug로 직접 확인한다. 실제 5초·30초 경과 또는 OS TCP 연결 지연을 실측한 테스트라고 주장하지 않는다. 본문 상한은 가짜 서버로 정확히 8MiB 허용·다음 1바이트 거부를 확인한다. 오류 매핑 연결은 알려진 코드와 서로 다른 진단 ID를 독립 결과로 단언한다.

외부 소비자 컴파일 실험은 현재 소스 경로를 의존하는 독립 crate로 실행한다. 작업 공간 잠금 사본을 쓰고 offline cargo check로 컴파일해 파일 수정 시각으로 rlib을 고르지 않는다.

## 추가 인계

- T-12/T-13: 새 계약은 toss-smoke의 직접 production 참조와 호스트 리터럴을 허용하지 않는다. 현재 toss-smoke는 루프백만 구성할 수 있다. 실제 연결 명령을 만들기 전에 메인이 별도 조립 진입점을 승인할지 또는 앱 조립 경로를 재사용할지 결정하고 해당 작업의 문서·구조 검사 계약을 함께 갱신해야 한다. 기존 tasks H-22와 ADR 0007 문구도 그때 정합화한다. 이번 작업은 승인된 한 파일 제한을 임의로 넓히지 않는다.
- T-03: 현재 Transport는 원본 오류나 URL·본문을 보관하지 않는 닫힌 전송 실패다. 재시도/사용자 표시를 구현할 때 보안 실패와 일시 장애의 추가 분류가 필요한지 검토한다. 현 범위에 재시도 정책을 만들지 않는다.

## 보강 손 변형 기록

추가 구조 검사 손 변형의 최초 실행은 검출 4, 생존 2, 컴파일 불가 1이었다. 두 경로 상수는 테스트가 구현 상수를 공유해 놓쳤다. 문서의 고정 경로를 독립 기대값으로 검사하도록 보강했고, 필드가 사용되지 않아 컴파일 불가였던 호스트 변형은 필드를 유지하는 잘못된 호스트 판정으로 바꿨다. 재실행 7개는 모두 컴파일 후 검출됐고 기준·복구도 통과했다. 컴파일 불가를 검출 수에 포함하지 않는다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 29 | 조립 파일 경로 교환 | arch_check::source::endpoint::tests | 검출 |
| 30 | 상수 허용 파일 경로 교환 | arch_check::source::endpoint::tests | 검출 |
| 31 | 매크로 생성자 위치 조건 반전 | arch_check::source::endpoint::tests | 검출 |
| 32 | 호스트 검사 문자열 교환 | arch_check::source::endpoint::tests | 검출 |
| 33 | 상수 이름 판정 반전 | arch_check::source::endpoint::tests | 검출 |
| 34 | 비공개 대신 공개 상수 허용 | arch_check::source::endpoint::tests | 검출 |
| 35 | 상수 파일 또는 조건을 그리고로 교환 | arch_check::source::endpoint::tests | 검출 |

## 리뷰 수정 손 변형과 좁은 검사

수정본에서 경로·쿼리·ID·포트 손 변형 28개를 다시 실행해 전부 컴파일 후 검출했고 기준/복구도 통과했다. 구조 검사 7개도 보강된 독립 경로 기대값을 포함한 현재 소스로 재실행해 전부 검출했다. 다음 8개는 리뷰 지적을 직접 깨뜨리는 추가 변형이며 모두 실제 컴파일 후 지정 테스트가 실패했다. 각 실행의 기준/복구 테스트도 통과했다. 고유 손 변형은 총43개다. 동일 변형 재실행을 새 개수로 세지 않는다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 36 | 제품 HTTPS 제한 해제 | product_configuration_rejects_plaintext_before_connecting | 검출 |
| 37 | 연결 제한 5초를 1초로 변경 | actual_builder_has_the_documented_timeouts_and_http_version | 검출 |
| 38 | 전체 제한 30초를 1초로 변경 | actual_builder_has_the_documented_timeouts_and_http_version | 검출 |
| 39 | 정확히 8MiB 거부 | body_limit_includes_exactly_eight_mib_and_rejects_the_next_byte | 검출 |
| 40 | 8MiB 다음 바이트 허용 | body_limit_includes_exactly_eight_mib_and_rejects_the_next_byte | 검출 |
| 41 | 비밀 헤더 sensitive 해제 | secret_headers_are_sensitive_and_reject_line_injection | 검출 |
| 42 | 오류 본문을 항상 Missing으로 바꿈 | raw_error_body_preserves_known_code_and_distinct_diagnostic_ids | 검출 |
| 43 | 추가 생성자와 상수 사용 검사를 무력화 | secondary_constructors_and_origin_exports_are_rejected | 검출 |

수정 후 broker-toss nextest 57/57, xtask endpoint 검사 5/5, 기존 구조 고정 자료 및 규칙 요소 제거 검사 6/6이 통과했다. 관련 clippy, source-check, arch-check도 통과했다. 이 결과를 다음 정상 커밋 전체 검사나 독립 재리뷰와 혼동하지 않는다.

## 리뷰 2 수정 결정

정식 생성자 내부의 중첩 impl이 방문자의 권한 상태를 상속하는 반례를 재현하는 회귀가 수정 전 실패했다. Item 경계에서 권한을 초기화하고, TossEndpoint의 직접 구현은 production/local/origin/mode의 현재 서명으로만 닫았다. 연관 상수·trait 생성자·별칭과 Debug 외 derive, 허용 밖 구조체 조립도 거부한다. 이 계약은 새로운 생성 경로를 임의로 추가하지 못하게 하는 보호된 구조 검사이며 Rust의 friend 가시성이라고 주장하지 않는다.

루프백 origin은 Client 생성 시에도 숫자 loopback IP, HTTP, 유효 포트, 빈 사용자 정보, 루트 경로 및 query/fragment 없음으로 다시 검증한다. local(port)는 여전히127.0.0.1만 만들고0을 거부한다. HTTP 기본 포트80은 URL 정규화로 port()가 사라지므로 알려진 기본 포트까지 검증해 유효한80을 보존한다.

프록시 프로브는 자식 libtest의1개 실행/1개 통과를 확인한다. 자체 비밀 인코딩·결합 버퍼는 필요한 용량을 미리 할당한다. AWS-LC와 macOS 활성 패키지는 현재 normal/build 그래프로 별도 표시하며 선택적·다른 대상 항목을 비활성으로 구분한다. 양성 고정 자료 이름을 새 계약에 맞추고 중첩/임의 origin 생성자의 음성 cases 자료를 추가했다.

## 회차2 수정 후 추가 손 변형

앞선 고유43개에 아래7개를 더해 총50개를 실제 컴파일 후 검출했다. mktemp 복사본에서 기준/복구 테스트도 통과했다. 동일 변형의 재실행은 개수에 더하지 않는다. 숫자 IP 판정을 단순 삭제한 첫 시도는 미사용 변수로 컴파일되지 않아 제외하고, 판정 반전 변형으로 검증했다. 기존 구조 검사7개도 현재 검사 소스로 재실행해 전부 검출했다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 44 | 숫자 IP 루프백 판정 반전 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |
| 45 | 평문 스킴 제한 제거 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |
| 46 | 0 포트 거부 제거 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |
| 47 | 경로 제한 제거 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |
| 48 | 쿼리 제한 제거 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |
| 49 | fragment 제한 제거 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |
| 50 | 프록시 프로브가 테스트0개 실행 | proxy_environment_is_ignored_in_a_fresh_process | 검출 |

최초 master 수신 검사에서 전체1431/1431, 커버리지 대상1043/1043 및 coverage-check 위반 없음까지 통과했으나 실행 중 master가 전진해 마지막 test-diff가 이전 MERGE_HEAD의 기준 예외를 거부했다. 실패한 검사를 통과로 세지 않았다. 회차2 소유 경로의 트리를 보존한 뒤 미완료 병합을 취소하고 최신 master를 다시 받아 정상 훅으로 검증한다. master ref나 검사 규칙을 바꾸지 않았다.

## 리뷰 3 수정 결정

회차3 Opus는 중요2/사소1을 지적했다. 중요 항목의 반례 쓰기가 리뷰어 훅에 막혀 추정으로 보고됐다. 구현 담당은80fdb66의 별도 git archive 복사본에서 자식 모듈 생성자, 주소 변경 자유 함수, from_url 클라이언트 생성자를 추가해 외부 소비자 테스트를 실제 실행했다. 컴파일·arch-check·source-check·raw-types가 모두 통과해 두 경로를 확정했다. 실제 요청은 보내지 않았다. 새 검사로 같은 복사본을 검사하면 endpoint.rs, endpoint/sneak.rs, http.rs가 모두 위반으로 잡힌다.

수정은 endpoint/http 모듈의 자식 모듈을 비공개 인라인 cfg(test) tests 하나로 닫고, 완전 수식·원시 식별자의 타입 경로도 정규화한다. 별칭 타입의 필드 조립, 자유 함수/상수의 필드 변경, 매크로로 감춘 선언도 거부한다. TossHttpClient의 필드는 모듈 비공개이며 직접 조립은 http.rs의 정식 new 안에만 허용한다. new/get/고정 send_token 외 클라이언트 구현을 거부한다. 클라이언트 내부 상태를 반환하는 다른 구현의 접근도 검사한다. 다른 모듈의 직접 조립은 별도 실제 Rust 컴파일에서 E0451로 실패했다. 기존 HTTPS 시험은 봉인을 뚫어 클라이언트를 조립하는 대신 실제 제품 빌더로 열린 루프백에 요청해 builder 거부·1초 이내 종료·연결0건을 단언한다. 기존 단언은 유지했다.

토큰 발급의 외부 서명과 form 생성은 그대로이고 고정 POST만 private 필드에 접근하는 http.rs 내부 메서드로 옮겼다. HTTP 쓰기 허용은 이 작업의 두 파일(token.rs의 토큰 준비, http.rs의 고정 발급 전송)에 한정한다. 기존 token.rs 고정 자료와 그 기대값은 보존하고 새 http.rs 양성 자료로 추가 위치의 필요성을 검증한다. pkg-config 허용 사유는 실제 aws-lc-sys 빌드 전이로 고쳤다. 새 패키지나 기능은 더하지 않았다.

이번 작업의 endpoint 검사 테스트 중 only_the_named_origin_constant_can_contain_the_host의 자유 함수 본문 바이트 문자열 사례는 Host 단일 기대값에서 Host+Production으로 강화했다. macros_escapes_c_strings_and_nested_constants_cannot_copy_the_host의 자유 함수3개도 같은 추가 위반을 정확히 단언한다. 기존 공개 상수 Host 사례는 별도 단언으로 보존했다. 외부 소비자 사례는6개에서7개로 늘려 from_url도 컴파일 실패로 고정했다. 허용한 모듈 비공개 필드·시험 모듈·완전 수식 정식 생성자의 양성 대조도 추가했다. 단언 삭제·와일드카드·검사 끄기는 없다.

Opus 리뷰는 승인된 최대3회에서 멈춘다. 마지막 지적은 새 Codex 독립 비판에서 수정 전 반례와 수정 후 회귀 근거를 확인했다. 회차3 Opus가 중요0이라고 보고한 것으로 바꾸지 않는다.

## 최종 자동 변이 범위

80fdb66에서 just mutants-diff dc8f3f950294e9d3d1947b39c256868076d7de15을 실제 실행해 종료0을 확인했다. catalog.json은 빈 배열이었다. 현재 자동 변이 설정은 domain/order-execution/application만 대상으로 하므로 T-18 adapter/xtask의 변이가 선택되지 않았다. 자동 검출 성공으로 보고하지 않으며 아래 실제 손 변형이 이 범위의 보완 증거다.

## 추가 경계 손 변형

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 51 | 기본 포트80 정규화 무시 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |
| 52 | 빈 사용자 이름의 비밀번호 검사 제거 | loopback_validation_rejects_forged_nonlocal_origins_and_url_credentials | 검출 |

기준과 복구가 모두 통과했다. 기본 포트80과 빈 사용자 이름/비밀번호 URL의 독립 기대 단언을 제품 시험 파일에도 반영했다.

## 회차3 수정 후 손 변형

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 53 | 모듈 비공개 필드 판정을 반전 | arch_check::source::endpoint | 검출 |
| 54 | 완전 수식 구현 경로 인식을 제거 | arch_check::source::endpoint | 검출 |
| 55 | 자식 모듈 검사 제거 | arch_check::source::endpoint | 검출 |
| 56 | 클라이언트 직접 조립 인식을 제거 | arch_check::source::endpoint | 검출 |
| 57 | 자유 필드 접근의 제품/시험 범위 교환 | arch_check::source::endpoint | 검출 |
| 58 | 클라이언트 닫힌 메서드 검사 제거 | arch_check::source::endpoint | 검출 |
| 59 | 환경 프록시 무시 제거 | proxy_environment_is_ignored_in_a_fresh_process | 검출 |

고유 손 변형은 총59개다. 위7개는 모두 실제 컴파일 후 검출했고 기준/복구도 통과했다. 최초 필드 가시성 반전이 좁은 시험에서 생존해 비공개 필드 양성 대조를 추가했다. 최초 메서드 검사 제거는 보조 함수 미사용으로 컴파일되지 않아 세지 않고, 참조를 유지한 무력화로 재실행했다. 중간 실행의 소스 문자열 불일치로 중단된 프로브도 완주로 세지 않았다. 최종6개 구조 변형은 모두 재실행했고, HTTPS/타임아웃/8MiB/헤더 민감성 기존6개도 봉인 수정 후 재실행해 검출했다.

수정 회차3의 첫 정상 훅 전체 테스트는1438개 중1250개 실행 시점에 기존 Git 훅 통합 시험5개가 실패해 중단됐다. 하위 commit-base/commit-guard의 signal9가 관찰됐고188개는 실행되지 않았다. 소스 변경 없이 실패한5개를 좁게 다시 실행해5/5 통과했다. 원인은 확정하지 않았다. 빌드 병렬도만 CARGO_BUILD_JOBS=2로 제한해 동일 정상 훅 전체 검사를 다시 실행한다. 테스트 수·속성 반복 수·검사 규칙은 줄이지 않는다.

## 최종 평가

04bcb89d02b0ae6841a99bd3cabddbe74bf71f30의 정상 훅 전체 검사는1438/1438, 커버리지 대상1048/1048이 통과했고 coverage-check·test-diff 및 cargo-deny 네 검사는 모두 위반이 없었다. 빌드 병렬도2를 사용했으며 속성 반복 수와 시험 선택은 줄이지 않았다. 앞선 signal9 실패의 원인은 확정하지 않는다.

새 Codex 독립 비판은 회차3의 자식 모듈·retarget·생성식 없는 완전 수식 필드 변경 및 클라이언트 from_url 공백을 중요로 확정하고 수정 후 해당 잔여0을 확인했다. 다만 제시된 완전 수식 any+Self 조립 예시는 수정 전에도 검출돼 그 부분은 기각했다. 두 Git blob에서 검사 제품 부분을 바이트 일치 추출해 동일 AST 입력을 실행했고 독립 Rust 자식 모듈 접근 프로브도 실행했다. 비판의 결과를 Opus 회차3의 원 집계가 중요0이었던 것으로 바꾸지 않는다.

비판자의 독립 검사는 broker57개(쓰기 경로 제한 때문에 http_surface 제외), endpoint8개, 등록1개와 신규 양성/음성 케이스, arch/source/raw/disable/deny다. 외부 소비자7개 컴파일 사례와 broker58개 전체는 부모 정상 훅에서 확인했다. 비판자는 전체 crate 삽입·전송 재현, no_proxy 제거, 전체 검사·자동 변이·실제 TLS를 수행하지 않았다. 증거의 소유자와 범위를 구분한다.

Production 모드의 origin을 런타임에서 상수와 다시 대조하는 장치는 없다. 정식 생성자·필드 비공개성 및 보호된 구조 검사로 이번 생성/변경 반례를 막는다. 이 제한은 Rust friend 가시성의 컴파일 차단이 아니다. 고정 생성자와 단일 주소 상수의 호출 위치를 구조 검사로 강제한다는 메인 계약을 따른다.

04bcb89에서도 just mutants-diff dc8f3f950294e9d3d1947b39c256868076d7de15을 실행해 종료0·catalog 빈 배열을 확인했다. 자동 설정 범위 밖의 adapter/xtask에 대해 자동 검출 성공을 주장하지 않는다. 고유59개 실제 컴파일/검출 손 변형 표가 그 범위의 보완 증거다.

검증하지 않은 것은 실제 Toss TLS 연결과 플랫폼 인증서 실패 동작, 실제5초/30초 시간 경과, Linux 실행이다. 실제 키·계좌·API는 사용하지 않았다. T-02 토큰 관리, T-03 제한/재시도 및 전송 오류 분류, T-05 이후 파싱·포트 구현, T-12/T-13 조립 진입점 정책과 문서 정합화, T-20 녹화 계약 확장은 위 인계대로 후속 작업에 남긴다.
