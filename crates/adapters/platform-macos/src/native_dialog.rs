use crate::order_presence::{OrderDialogBackend, OrderDialogContent, OrderDialogResponse};
use crate::risk_dialog::{RiskChangeDialogContent, RiskDialogBackend, RiskDialogResponse};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogChoice {
    Confirmed,
    Cancelled,
}

#[must_use]
pub fn choice_from(result: &MessageDialogResult, confirm_label: &str) -> DialogChoice {
    match result {
        MessageDialogResult::Ok | MessageDialogResult::Yes => DialogChoice::Confirmed,
        MessageDialogResult::Custom(label) if label == confirm_label => DialogChoice::Confirmed,
        MessageDialogResult::No | MessageDialogResult::Cancel | MessageDialogResult::Custom(_) => {
            DialogChoice::Cancelled
        }
    }
}

fn ask(title: &str, body: &str, confirm: &'static str, cancel: &'static str) -> DialogChoice {
    let result = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title(title)
        .set_description(body)
        .set_buttons(MessageButtons::OkCancelCustom(
            String::from(confirm),
            String::from(cancel),
        ))
        .show();
    choice_from(&result, confirm)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeRiskDialog;

impl RiskDialogBackend for NativeRiskDialog {
    fn present(&self, content: &RiskChangeDialogContent) -> RiskDialogResponse {
        match ask(
            &content.title,
            &content.body,
            content.confirm_label,
            content.cancel_label,
        ) {
            DialogChoice::Confirmed => RiskDialogResponse::Confirmed,
            DialogChoice::Cancelled => RiskDialogResponse::Cancelled,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeOrderDialog;

impl OrderDialogBackend for NativeOrderDialog {
    fn present(&self, content: &OrderDialogContent) -> OrderDialogResponse {
        match ask(
            &content.title,
            &content.body,
            content.confirm_label,
            content.cancel_label,
        ) {
            DialogChoice::Confirmed => OrderDialogResponse::Confirmed,
            DialogChoice::Cancelled => OrderDialogResponse::Cancelled,
        }
    }
}

pub const EXPORT_DIALOG_TITLE: &str = "백업 파일을 저장할 위치";
pub const EXPORT_FILTER_NAME: &str = "Stock Copilot 백업";
pub const EXPORT_EXTENSION: &str = "db";

#[must_use]
pub fn pick_save_path(file_name: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title(EXPORT_DIALOG_TITLE)
        .add_filter(EXPORT_FILTER_NAME, &[EXPORT_EXTENSION])
        .set_file_name(file_name)
        .save_file()
}

#[cfg(test)]
mod tests {
    use super::{DialogChoice, choice_from};
    use rfd::MessageDialogResult;

    #[test]
    fn only_the_confirm_button_confirms() {
        for (result, expected) in [
            (
                MessageDialogResult::Custom(String::from("주문")),
                DialogChoice::Confirmed,
            ),
            (
                MessageDialogResult::Custom(String::from("취소")),
                DialogChoice::Cancelled,
            ),
            (MessageDialogResult::Cancel, DialogChoice::Cancelled),
            (MessageDialogResult::No, DialogChoice::Cancelled),
            (MessageDialogResult::Ok, DialogChoice::Confirmed),
        ] {
            assert_eq!(choice_from(&result, "주문"), expected, "{result:?}");
        }
    }
}
