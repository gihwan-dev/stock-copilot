use crate::platform::residency::{ResidencyPresence, residency_presence};
use crate::platform::tray::{
    TrayIconState, TrayMenuAction, action_code, action_of, residency_tray_menu, template_icon_rgba,
};

#[test]
fn the_menu_bar_offers_open_sync_status_and_quit_in_that_order() {
    let items = residency_tray_menu();
    let labels: Vec<&str> = items.iter().map(|item| item.label).collect();
    assert_eq!(labels, ["열기", "지금 동기화", "데이터 상태", "종료"]);
    for item in items {
        assert_eq!(action_of(action_code(item.action)), Some(item.action));
    }
    assert_eq!(action_of("unknown"), None);
    assert_eq!(
        items.map(|item| item.action),
        [
            TrayMenuAction::OpenMain,
            TrayMenuAction::SyncNow,
            TrayMenuAction::DataStatus,
            TrayMenuAction::Quit,
        ]
    );
}

#[test]
fn the_icon_is_a_black_alpha_template_and_the_dock_follows_the_tray() {
    let (rgba, width, height) = template_icon_rgba();
    assert_eq!((width, height), (32, 32));
    assert_eq!(rgba.len(), 4096);
    let (pixels, rest) = rgba.as_chunks::<4>();
    assert_eq!(rest.len(), 0);
    assert!(
        pixels
            .iter()
            .all(|[red, green, blue, _alpha]| [*red, *green, *blue] == [0, 0, 0])
    );
    let opaque = pixels
        .iter()
        .filter(|[_red, _green, _blue, alpha]| *alpha == 255)
        .count();
    assert!(opaque > 100 && opaque < 600, "{opaque}");
    assert_eq!(
        residency_presence(TrayIconState::Visible),
        ResidencyPresence::MenuBarOnly
    );
    assert_eq!(
        residency_presence(TrayIconState::Hidden),
        ResidencyPresence::DockIcon
    );
}
