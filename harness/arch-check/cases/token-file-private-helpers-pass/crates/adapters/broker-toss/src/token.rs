use reqwest::Method;

const TOKEN_PATH: &str = "/oauth2/token";

fn call(client: &Client, verb: Method, url: &str) -> RequestBuilder {
    client.request(verb, url)
}

pub(crate) fn issue(client: &Client) -> Token {
    call(client, Method::POST, TOKEN_PATH).send()
}
