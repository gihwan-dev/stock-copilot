const ORDERS: &str = "/api/v1/orders";
const CONDITIONAL: &str = "/api/v1/conditional-orders";

pub(crate) fn list(client: &Client) {
    client.get(ORDERS);
    client.get(CONDITIONAL);
}

pub(crate) fn one(client: &Client, id: &str) {
    client.get(format!("{ORDERS}/{id}"));
}
