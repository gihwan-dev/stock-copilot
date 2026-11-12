pub(crate) fn place(client: &Client) {
    call(client, Verb::POST, "/api/v1/orders").send();
}
