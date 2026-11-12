pub(crate) fn place(client: &Client) {
    client.post("/api/v1/orders");
}
