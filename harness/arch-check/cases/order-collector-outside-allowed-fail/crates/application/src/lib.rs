mod synthesized;

pub fn forge(scope: Scope) {
    let _ = domain::order_query_proof::r#ClosedOrderPageCollector::start(scope);
}
