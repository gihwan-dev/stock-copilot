fn main() {}

fn modify(client: &Client, id: &str) {
    client.put(format!("/api/v1/orders/{id}/modify"));
}
