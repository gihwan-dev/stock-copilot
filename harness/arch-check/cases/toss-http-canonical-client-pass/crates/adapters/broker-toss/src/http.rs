pub struct TossHttpClient { client: Client, origin: Url }
impl TossHttpClient {
    pub fn new(endpoint: &TossEndpoint, clock: Arc<dyn Clock>, wait: Arc<dyn crate::RequestWait>) -> Result<Self, TossHttpError> { Ok(Self { client: client(), origin: origin(endpoint) }) }
    pub(crate) async fn send_token(&self, body: SecretValue) -> Result<RawResponse, TossHttpError> { self.client.post("/oauth2/token").body(body).send().await }
}
