pub(crate) fn send(client: &Client) {
    client.request(Method::DELETE, format!("{BASE}{TARGET}/{}", 1));
}
