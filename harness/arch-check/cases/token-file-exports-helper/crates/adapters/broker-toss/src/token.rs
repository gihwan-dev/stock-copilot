pub(crate) fn call(client: &Client, verb: Method, url: &str) -> RequestBuilder {
    client.request(verb, url)
}
