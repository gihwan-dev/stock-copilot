pub(crate) fn submit(client: &Client) {
    client.post("/api/v1/orders");
}
