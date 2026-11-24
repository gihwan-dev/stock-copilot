# O 묶음 경량 결정 기록 (ADR 0173)

문서가 세부 값을 정하지 않은 선택지를 가장 안전한 쪽(주문이 나가지 않는 쪽)으로 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| O-03 | `UserApproval`과 `ApprovedOrder`가 `domain::PendingOrderConfirmation`을 소유해 `Proposal<Validated>`와 사용자가 본 `OrderSummary`가 분리되지 않게 한다. `PendingOrderConfirmation`에는 분해 없는 읽기 접근자만 추가하고 `into_execution_parts`는 `pub(crate)`를 유지한다. `OrderOutcome::Unknown`은 `Maintenance`가 아닐 때만 원본 `ApprovedOrder`와 최초 전송 시각을 봉인한 `UnknownOrderRetry`를 반환하고, `Accepted`·`Rejected`·`Maintenance`는 `ApprovedOrder`를 즉시 소멸시킨다. | `UserApproval`이 `ExecutionToken`과 `OrderSummary`를 별도 필드로 분해해 소유하는 방식, 또는 `OrderOutcome::Unknown`에서 원시 주문 재구성 허용 |
