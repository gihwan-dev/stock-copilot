use super::HistorySkips;

#[test]
fn a_clean_history_read_leaves_no_diagnostic() {
    assert_eq!(HistorySkips::default().summary(), None);
}

#[test]
fn only_dropped_rows_are_reported() {
    let dropped = HistorySkips {
        unreadable: 2,
        future: 0,
        candle: 1,
    };
    assert_eq!(
        dropped.summary().as_deref(),
        Some("history-skipped unreadable=2 future=0 candle=1")
    );
    let future = HistorySkips {
        unreadable: 0,
        future: 1,
        candle: 0,
    };
    assert_eq!(
        future.summary().as_deref(),
        Some("history-skipped unreadable=0 future=1 candle=0")
    );
}
