fn hex_digit(nibble: u8) -> char {
    char::from(if nibble < 10 {
        b'0'.saturating_add(nibble)
    } else {
        b'A'.saturating_add(nibble.saturating_sub(10))
    })
}

use crate::http::receive;
use crate::{RawResponse, TossHttpClient, TossHttpError};
use application::SecretValue;

fn encode(value: &[u8]) -> String {
    let mut encoded = String::new();
    for byte in value {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(*byte));
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push('%');
                for nibble in [byte >> 4_u32, byte & 15] {
                    encoded.push(hex_digit(nibble));
                }
            }
        }
    }
    encoded
}

pub async fn issue_token(
    client: &TossHttpClient,
    client_id: &SecretValue,
    client_secret: &SecretValue,
) -> Result<RawResponse, TossHttpError> {
    let mut url = client.origin.clone();
    url.set_path("/oauth2/token");
    let body = format!(
        "grant_type=client_credentials&client_id={}&client_secret={}",
        encode(client_id.as_exposed_secret()),
        encode(client_secret.as_exposed_secret())
    );
    receive(
        client
            .client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|_error| TossHttpError::Transport)?,
    )
    .await
}
