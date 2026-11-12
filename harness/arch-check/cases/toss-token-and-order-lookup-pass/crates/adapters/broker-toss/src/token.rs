const TOKEN_PATH: &str = "/oauth2/token";

pub(crate) fn issue(client: &Client) {
    client.post(TOKEN_PATH);
}
