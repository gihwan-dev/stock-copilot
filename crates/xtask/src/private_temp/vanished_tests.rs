use std::fs;
use std::thread;

use super::{PrivateDirectory, modified_unless_gone, remove_unless_gone};

const CHURN_PREFIX: &str = "stock-copilot-private-churn-test";
const WORKERS: usize = 8;
const ROUNDS: usize = 40;

fn churn_failures() -> usize {
    (0..ROUNDS)
        .filter(|_| {
            PrivateDirectory::create(CHURN_PREFIX)
                .and_then(PrivateDirectory::remove)
                .is_err()
        })
        .count()
}

#[test]
fn entries_removed_by_another_process_are_skipped() {
    let directory = PrivateDirectory::create("stock-copilot-private-vanish-test").expect("create");
    let gone = directory.root.join("gone");
    assert_eq!(modified_unless_gone(&gone), Ok(None));
    assert_eq!(remove_unless_gone(&gone), Ok(()));
    fs::create_dir_all(&gone).expect("create gone");
    assert!(
        modified_unless_gone(&gone).is_ok_and(|modified| modified.is_some()),
        "present folder has a time"
    );
    assert_eq!(remove_unless_gone(&gone), Ok(()));
    directory.remove().expect("remove");
}

#[test]
fn creating_while_others_come_and_go_under_the_same_prefix_never_fails() {
    let failures: usize = thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..WORKERS {
            workers.push(scope.spawn(churn_failures));
        }
        workers
            .into_iter()
            .map(|worker| worker.join().expect("worker"))
            .sum()
    });
    assert_eq!(failures, 0);
}
