use tauri_specta::internal::_tauri_specta_paste as synth;

pub fn forge() {
    synth! {
        impl application::[<identity_confirmation _prompt>]::[<IdentityConfirmation Prompt>] for Auto {}
    }
}
