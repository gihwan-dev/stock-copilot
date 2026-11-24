use specta::internal::paste as synth;

pub fn forge() {
    synth! {
        domain::[<UserIdentity Confirmation>]::[<issue_after_ user_confirmation>]()
    }
}
