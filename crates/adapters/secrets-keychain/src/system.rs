use crate::{AccessGroup, KeychainBackend, KeychainFailure, KeychainItem, KeychainKind};
use security_framework::access_control::{ProtectionMode, SecAccessControl};
use security_framework::base::Error;
use security_framework::passwords::{
    PasswordOptions, delete_generic_password_options, generic_password,
    set_generic_password_options,
};

const ITEM_NOT_FOUND: i32 = -25_300;
const MISSING_ENTITLEMENT: i32 = -34_018;
const INTERACTION_NOT_ALLOWED: i32 = -25_308;
const AUTH_FAILED: i32 = -25_293;
const USER_CANCELED: i32 = -128;
const DECODE: i32 = -26_275;
const PARAM: i32 = -50;

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemKeychain;

#[derive(Clone, Copy)]
enum Purpose {
    Query,
    Insert,
}

fn options(item: &KeychainItem, purpose: Purpose) -> Result<PasswordOptions, KeychainFailure> {
    let mut options =
        PasswordOptions::new_generic_password(item.service().as_str(), item.account());
    match item.kind() {
        KeychainKind::Login => {}
        KeychainKind::DataProtection => {
            options.use_protected_keychain();
            if let AccessGroup::Named(group) = item.access_group() {
                options.set_access_group(group);
            }
            match purpose {
                Purpose::Query => {}
                Purpose::Insert => {
                    options.set_access_synchronized(Some(false));
                    let control = SecAccessControl::create_with_protection(
                        Some(ProtectionMode::AccessibleAfterFirstUnlockThisDeviceOnly),
                        0,
                    )
                    .map_err(failure)?;
                    options.set_access_control(control);
                }
            }
        }
    }
    Ok(options)
}

const fn failure(error: Error) -> KeychainFailure {
    match error.code() {
        MISSING_ENTITLEMENT => KeychainFailure::MissingEntitlement,
        INTERACTION_NOT_ALLOWED => KeychainFailure::Locked,
        AUTH_FAILED | USER_CANCELED => KeychainFailure::Denied,
        DECODE | PARAM => KeychainFailure::Malformed,
        _ => KeychainFailure::Unavailable,
    }
}

fn absent_is_ok(result: Result<(), Error>) -> Result<(), KeychainFailure> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.code() == ITEM_NOT_FOUND => Ok(()),
        Err(error) => Err(failure(error)),
    }
}

impl KeychainBackend for SystemKeychain {
    fn read(&self, item: &KeychainItem) -> Result<Option<Vec<u8>>, KeychainFailure> {
        match generic_password(options(item, Purpose::Query)?) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
            Err(error) => Err(failure(error)),
        }
    }

    fn write(&self, item: &KeychainItem, value: &[u8]) -> Result<(), KeychainFailure> {
        self.delete(item)?;
        set_generic_password_options(value, options(item, Purpose::Insert)?).map_err(failure)
    }

    fn delete(&self, item: &KeychainItem) -> Result<(), KeychainFailure> {
        absent_is_ok(delete_generic_password_options(options(
            item,
            Purpose::Query,
        )?))
    }
}
