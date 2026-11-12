pub(crate) fn place(client: &Client, path: &str) {
    let verb = Method::from_bytes(&[80, 79, 83, 84]);
    client.request(verb, path);
}
