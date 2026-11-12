pub(crate) fn send(client: &Client, url: &str) {
    client.patch(url);
}
