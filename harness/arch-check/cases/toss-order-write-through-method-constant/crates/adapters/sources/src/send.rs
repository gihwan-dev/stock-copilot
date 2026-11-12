fn send(client: &Client) {
    client.call(VERB, "/api/v1/conditional-orders/7/modify");
}
