pub(crate) fn place(client: &Client) {
    client.post(concat!("/api/v1/", "ord", "ers"));
}
