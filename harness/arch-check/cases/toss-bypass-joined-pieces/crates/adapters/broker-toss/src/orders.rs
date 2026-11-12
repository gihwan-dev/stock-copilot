pub(crate) fn place(client: &Client) {
    let path = ["/api/v1/", "ord", "ers"].concat();
    client.post(path);
}
