fn hex_digit(nibble: u8) -> u8 {
    if nibble < 10 {
        b'0'.saturating_add(nibble)
    } else {
        b'A'.saturating_add(nibble.saturating_sub(10))
    }
}

use crate::http::receive;
use crate::{RawResponse, TossHttpClient, TossHttpError};
use application::SecretValue;

fn encode(value: &[u8]) -> SecretValue {
    let mut encoded = Vec::new();
    for byte in value {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(*byte);
            }
            b' ' => encoded.push(b'+'),
            _ => {
                encoded.push(b'%');
                for nibble in [byte >> 4_u32, byte & 15] {
                    encoded.push(hex_digit(nibble));
                }
            }
        }
    }
    SecretValue::new(encoded)
}

pub async fn issue_token(
    client: &TossHttpClient,
    client_id: &SecretValue,
    client_secret: &SecretValue,
) -> Result<RawResponse, TossHttpError> {
    let mut url = client.origin.clone();
    url.set_path("/oauth2/token");
    let id = encode(client_id.as_exposed_secret());
    let secret = encode(client_secret.as_exposed_secret());
    let body = SecretValue::new(
        [
            b"grant_type=client_credentials&client_id=",
            id.as_exposed_secret(),
            b"&client_secret=",
            secret.as_exposed_secret(),
        ]
        .concat(),
    );
    receive(
        client
            .client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.as_exposed_secret().to_vec())
            .send()
            .await
            .map_err(|_error| TossHttpError::Transport)?,
    )
    .await
}
