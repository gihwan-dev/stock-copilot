pub(crate) fn send(client: &Client, verb: Verb, url: &str) {
    client.request(verb, url);
}
