pub(crate) fn send(client: &Client, url: &str) {
    client.delete(url);
}
