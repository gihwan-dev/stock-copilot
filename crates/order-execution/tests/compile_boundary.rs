#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Output};

    type CompileCase = (&'static str, &'static str, Option<&'static str>);

    fn run(directory: &Path, source: &str) -> Output {
        fs::write(directory.join("src/main.rs"), source).expect("소스 쓰기");
        testkit::check_consumer(&directory.join("Cargo.toml"), &directory.join("target"))
            .expect("컴파일러 실행")
    }

    fn seal_cases() -> [CompileCase; 6] {
        [
            (
                "control_signature",
                "use order_execution::{ApprovedOrder, BrokerTrade, OrderOutcome, UserApproval}; fn check<B: BrokerTrade>(broker: &B, order: ApprovedOrder) { let _ = broker.submit(order); } fn main() { let _ = check::<Dummy>; let _: Option<UserApproval> = None; } struct Dummy; impl BrokerTrade for Dummy { async fn submit(&self, order: ApprovedOrder) -> OrderOutcome { order.into_unknown(order_execution::OrderUnknownCause::Maintenance) } }",
                None,
            ),
            (
                "forge_user_approval_literal",
                "use domain::PendingOrderConfirmation; use order_execution::{ApprovalSignatureSlot, UserApproval}; fn forge(confirmation: PendingOrderConfirmation) -> UserApproval { UserApproval { confirmation, signature: ApprovalSignatureSlot::PendingSecureEnclave } } fn main() { let _ = forge; }",
                Some("error[E0451]"),
            ),
            (
                "forge_approved_order_literal",
                "use domain::PendingOrderConfirmation; use order_execution::{ApprovalSignatureSlot, ApprovedOrder}; fn forge(confirmation: PendingOrderConfirmation) -> ApprovedOrder { ApprovedOrder { confirmation, signature: ApprovalSignatureSlot::PendingSecureEnclave } } fn main() { let _ = forge; }",
                Some("error[E0451]"),
            ),
            (
                "private_new_internal",
                "use domain::PendingOrderConfirmation; use order_execution::{ApprovalSignatureSlot, UserApproval}; fn forge(confirmation: PendingOrderConfirmation) -> UserApproval { UserApproval::new_internal(confirmation, ApprovalSignatureSlot::PendingSecureEnclave) } fn main() { let _ = forge; }",
                Some("error[E0624]"),
            ),
            (
                "reuse_user_approval_twice",
                "use order_execution::{ApprovedOrder, UserApproval}; fn twice(approval: UserApproval) -> (ApprovedOrder, ApprovedOrder) { let first = approval.into_order(); let second = approval.into_order(); (first, second) } fn main() { let _ = twice; }",
                Some("error[E0382]"),
            ),
            (
                "clone_user_approval",
                "use order_execution::UserApproval; fn dup(approval: &UserApproval) -> UserApproval { approval.clone() } fn main() { let _ = dup; }",
                Some("error[E0308]"),
            ),
        ]
    }

    fn misuse_cases() -> [CompileCase; 6] {
        [
            (
                "clone_approved_order",
                "use order_execution::ApprovedOrder; fn dup(order: &ApprovedOrder) -> ApprovedOrder { order.clone() } fn main() { let _ = dup; }",
                Some("error[E0308]"),
            ),
            (
                "default_user_approval",
                "use order_execution::UserApproval; fn make() -> UserApproval { <UserApproval as Default>::default() } fn main() { let _ = make; }",
                Some("error[E0277]"),
            ),
            (
                "default_approved_order",
                "use order_execution::ApprovedOrder; fn make() -> ApprovedOrder { <ApprovedOrder as Default>::default() } fn main() { let _ = make; }",
                Some("error[E0277]"),
            ),
            (
                "reuse_approved_order_twice",
                "use order_execution::{ApprovedOrder, BrokerTrade, OrderOutcome}; async fn twice<B: BrokerTrade>(broker: &B, order: ApprovedOrder) -> (OrderOutcome, OrderOutcome) { let first = broker.submit(order).await; let second = broker.submit(order).await; (first, second) } fn main() { let _ = twice::<Dummy>; } struct Dummy; impl BrokerTrade for Dummy { async fn submit(&self, order: ApprovedOrder) -> OrderOutcome { order.into_unknown(order_execution::OrderUnknownCause::Maintenance) } }",
                Some("error[E0382]"),
            ),
            (
                "submit_raw_intent_rejected",
                "use domain::OrderIntent; use order_execution::{ApprovedOrder, BrokerTrade, OrderOutcome}; async fn wrong<B: BrokerTrade>(broker: &B, intent: OrderIntent) -> OrderOutcome { broker.submit(intent).await } fn main() { let _ = wrong::<Dummy>; } struct Dummy; impl BrokerTrade for Dummy { async fn submit(&self, order: ApprovedOrder) -> OrderOutcome { order.into_unknown(order_execution::OrderUnknownCause::Maintenance) } }",
                Some("error[E0308]"),
            ),
            (
                "submit_pending_confirmation_rejected",
                "use domain::PendingOrderConfirmation; use order_execution::{ApprovedOrder, BrokerTrade, OrderOutcome}; async fn wrong<B: BrokerTrade>(broker: &B, pending: PendingOrderConfirmation) -> OrderOutcome { broker.submit(pending).await } fn main() { let _ = wrong::<Dummy>; } struct Dummy; impl BrokerTrade for Dummy { async fn submit(&self, order: ApprovedOrder) -> OrderOutcome { order.into_unknown(order_execution::OrderUnknownCause::Maintenance) } }",
                Some("error[E0308]"),
            ),
        ]
    }

    fn serialization_cases() -> [CompileCase; 4] {
        [
            (
                "serialize_user_approval",
                "use order_execution::UserApproval; fn check<T: serde_core::Serialize>() {} fn main() { check::<UserApproval>(); }",
                Some("error[E0277]"),
            ),
            (
                "deserialize_user_approval",
                "use order_execution::UserApproval; fn check<'de, T: serde_core::Deserialize<'de>>() {} fn main() { check::<UserApproval>(); }",
                Some("error[E0277]"),
            ),
            (
                "serialize_approved_order",
                "use order_execution::ApprovedOrder; fn check<T: serde_core::Serialize>() {} fn main() { check::<ApprovedOrder>(); }",
                Some("error[E0277]"),
            ),
            (
                "deserialize_approved_order",
                "use order_execution::ApprovedOrder; fn check<'de, T: serde_core::Deserialize<'de>>() {} fn main() { check::<ApprovedOrder>(); }",
                Some("error[E0277]"),
            ),
        ]
    }

    #[test]
    fn sealed_order_types_cannot_be_forged_cloned_serialized_or_reused() {
        let root = std::env::current_dir().expect("order-execution 경로");
        let directory = root
            .join("../../target/O-03")
            .join(format!("compile-boundary-{}", std::process::id()));
        fs::create_dir_all(directory.join("src")).expect("임시 폴더");
        let order_exec_path = root
            .canonicalize()
            .expect("정규 경로")
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let domain_path = root
            .join("../domain")
            .canonicalize()
            .expect("domain 경로")
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let manifest = format!(
            "[package]\nname = \"o03-boundary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\norder-execution = {{ path = \"{order_exec_path}\" }}\ndomain = {{ path = \"{domain_path}\" }}\nserde_core = {{ version = \"=1.0.229\", default-features = false }}\n"
        );
        fs::write(directory.join("Cargo.toml"), manifest).expect("명세 쓰기");
        for (name, source, expected) in seal_cases()
            .into_iter()
            .chain(misuse_cases())
            .chain(serialization_cases())
        {
            let output = run(&directory, source);
            let diagnostics = String::from_utf8_lossy(&output.stderr);
            match expected {
                None => assert!(output.status.success(), "{name}: {diagnostics}"),
                Some(code) => {
                    assert!(
                        !output.status.success(),
                        "{name} unexpectedly compiled: {diagnostics}"
                    );
                    assert!(diagnostics.contains(code), "{name}: {diagnostics}");
                }
            }
        }
        fs::remove_dir_all(directory).expect("임시 폴더 정리");
    }
}
