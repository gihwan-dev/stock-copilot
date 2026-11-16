use tokio as t;

pub async fn resolve() {
    let _addresses = t::net::lookup_host("example.invalid:443").await;
}
