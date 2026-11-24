# O 묶음 경량 결정 기록 (ADR 0173)

문서가 세부 값을 정하지 않은 선택지를 가장 안전한 쪽(주문이 나가지 않는 쪽)으로 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| O-03 | `UserApproval`과 `ApprovedOrder`가 `domain::PendingOrderConfirmation`을 소유해 `Proposal<Validated>`와 사용자가 본 `OrderSummary`가 분리되지 않게 한다. `PendingOrderConfirmation`에는 분해 없는 읽기 접근자만 추가하고 `into_execution_parts`는 `pub(crate)`를 유지한다. `OrderOutcome::Unknown`은 `Maintenance`가 아닐 때만 원본 `ApprovedOrder`와 최초 전송 시각을 봉인한 `UnknownOrderRetry`를 반환하고, `Accepted`·`Rejected`·`Maintenance`는 `ApprovedOrder`를 즉시 소멸시킨다. | `UserApproval`이 `ExecutionToken`과 `OrderSummary`를 별도 필드로 분해해 소유하는 방식, 또는 `OrderOutcome::Unknown`에서 원시 주문 재구성 허용 |
| O-01 | 장 시간 게이트에서 미국 금액 주문·소수 수량 매도는 정규장 시작부터 정규장 종료 1시간 전 미만(`[start, end - 1h)`)만 통과시키고, 일반 시장가 주문도 보수적으로 정규장(`RegularMarket`)만 통과시키며, 지정가 주문은 개장 세션 전체를 허용한다. 매수 가능 금액 검사는 유효한 수수료율 사실이 있으면 추정 수수료를 더한 금액과 비교하고, 매도 수량 검사는 매도가능수량 사실과 보유 수량 둘 다 이내여야 통과시킨다. 국내 주문은 가격 제한 사실이 없으면 `Unknown`으로 거부한다. | 시장가 주문을 프리·애프터마켓에서도 허용하거나, 수수료를 제외한 원금만으로 매수가능금액을 비교하는 방식 |
| O-22 | 전체 시간당 주문 건수 상한(ORD-6) 기본값을 **롤링 1시간(3600초)당 10건**(설정 허용 범위 1~30건)으로 정한다(토스 어뷰징 예 "수십 분 내 수백 건"보다 충분히 낮음). 미래 시각 전송 기록(시계 역행)이 있으면 보수적으로 거부(`ClockRollback`)한다. | 정시 단위 고정 창(버스트 허용) 또는 더 높은 상한(50~100건) |
| O-04 | 거절한 제안의 재요청 제한(쿨다운) 기본값을 **60초**(설정 허용 범위 1~3600초)로 정한다. 확인 창 동시성 제한은 `ApprovalGuard`의 RAII `DialogLease`로 강제해 호출 종료·패닉 시에도 1건 제약을 안전하게 해제한다. `ProposalExpired` 이벤트는 `ProposalId`당 최초 1회만 발행한다. | 더 짧은 쿨다운(5~15초) 또는 전역 쿨다운 |
