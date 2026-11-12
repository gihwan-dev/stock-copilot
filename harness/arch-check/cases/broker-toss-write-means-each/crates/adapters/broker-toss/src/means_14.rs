pub(crate) fn send(client: &Client, prepared: Prepared) {
    client.execute(prepared);
}
