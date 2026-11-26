use super::{UiAutoStartChoice, change_auto_start};
use crate::ipc::settings::UiToggleState;
use crate::platform::autostart::{
    AGENT_LABEL, AutostartPort, AutostartState, LOGIN_ARGUMENT, LaunchAgentAutostart,
    prepare_login_items,
};
use std::fs;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let directory =
        PathBuf::from("../../../target/U-19").join(format!("{name}-{}", std::process::id()));
    let _cleared = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("임시 폴더");
    directory
}

fn port(directory: &std::path::Path) -> LaunchAgentAutostart {
    LaunchAgentAutostart::in_home(
        &directory.join("home"),
        PathBuf::from("/Applications/Stock & Copilot.app"),
    )
}

#[test]
fn launch_agent_plist_opens_the_bundle_quietly_with_the_login_argument() {
    let directory = scratch("plist");
    let agent = port(&directory);
    assert!(
        agent
            .agent_path()
            .ends_with(format!("Library/LaunchAgents/{AGENT_LABEL}.plist"))
    );
    let plist = agent.plist();
    for expected in [
        "<string>/usr/bin/open</string>",
        "<string>-g</string>",
        "<string>/Applications/Stock &amp; Copilot.app</string>",
        "<key>RunAtLoad</key>",
        "<true/>",
    ] {
        assert!(plist.contains(expected), "{expected}");
    }
    assert!(plist.contains(&format!("<string>{LOGIN_ARGUMENT}</string>")));
    let _removed = fs::remove_dir_all(directory);
}

#[test]
fn first_run_turns_login_start_on_and_later_runs_keep_the_users_choice() {
    let directory = scratch("first-run");
    let data = directory.join("data");
    let items = prepare_login_items(port(&directory), &data).expect("첫 실행");
    assert_eq!(items.desired_state(), AutostartState::Enabled);
    assert_eq!(items.port().current(), Ok(AutostartState::Enabled));
    let events = application::EventBusBuilder::default().build();
    let view = change_auto_start(&items, &events, UiAutoStartChoice::Off).expect("끄기");
    crate::ipc::tests::assert_response_snapshot("set_auto_start", &view);
    assert_eq!(view.auto_start_at_login, UiToggleState::Off);
    assert!(!port(&directory).agent_path().exists());
    let again = prepare_login_items(port(&directory), &data).expect("다음 실행");
    assert_eq!(again.desired_state(), AutostartState::Disabled);
    assert_eq!(again.port().current(), Ok(AutostartState::Disabled));
    let on = change_auto_start(&again, &events, UiAutoStartChoice::On).expect("켜기");
    assert_eq!(on.auto_start_at_login, UiToggleState::On);
    assert!(port(&directory).agent_path().exists());
    let _removed = fs::remove_dir_all(directory);
}
