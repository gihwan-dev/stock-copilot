use application::{RetryDelay, SecretValue, SystemFailure, SystemPort, SystemPortError};
use security_framework::random::SecRandom;

const DATABASE_KEY_BYTES: usize = 32;

pub fn generate_database_key() -> Result<SecretValue, SystemPortError> {
    let mut bytes = vec![0_u8; DATABASE_KEY_BYTES];
    match SecRandom::default().copy_bytes(&mut bytes) {
        Ok(()) => Ok(SecretValue::new(bytes)),
        Err(_error) => Err(SystemPortError::new(
            SystemPort::SecretStore,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )),
    }
}
