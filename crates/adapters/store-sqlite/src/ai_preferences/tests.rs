use crate::{
    AiPreferencesStore, Database, DatabasePath,
    tests::{Fixture, clock, key, runtime},
};
use application::{
    AiEffortChoice, AiModelChoice, AiPreferences, AiPreferencesRepository, AiTask,
    AiTaskPreference, ClaudeConfigDir, ScheduledBudgetPercent,
};

#[test]
fn preferences_round_trip_through_sqlite_and_replace_the_single_row() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let database = Database::open(&path, &key(), &clock()).expect("open");
    let store = AiPreferencesStore::new(&database);
    assert_eq!(
        rt.block_on(store.load_ai_preferences()).expect("빈 값"),
        None
    );
    let mut preferences = AiPreferences::initial();
    preferences.set_task(AiTaskPreference {
        task: AiTask::Chat,
        model: AiModelChoice::Sonnet,
        effort: AiEffortChoice::XHigh,
    });
    preferences.set_budget(ScheduledBudgetPercent::Eighty);
    preferences.set_config_dir(Some(
        ClaudeConfigDir::try_new("/Users/me/claude config").expect("경로"),
    ));
    rt.block_on(store.store_ai_preferences(&preferences))
        .expect("저장");
    assert_eq!(
        rt.block_on(store.load_ai_preferences()).expect("읽기"),
        Some(preferences.clone())
    );
    preferences.set_config_dir(None);
    rt.block_on(store.store_ai_preferences(&preferences))
        .expect("덮어쓰기");
    assert_eq!(
        rt.block_on(store.load_ai_preferences()).expect("다시 읽기"),
        Some(preferences)
    );
}

#[test]
fn a_damaged_payload_is_refused_instead_of_defaulting() {
    for payload in [
        "",
        "budget:75\ndir:",
        "budget:70\ndir:\ntask:chat:gpt:low",
        "budget:70",
    ] {
        assert!(super::decode(payload).is_err(), "{payload}");
    }
    let initial = AiPreferences::initial();
    assert_eq!(
        super::decode(&super::encode(&initial)).expect("기본값"),
        initial
    );
}
