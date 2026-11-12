pub(crate) fn send(client: &Client, url: &str) {
    client.post(url);
}
