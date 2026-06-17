use reqwest::RequestBuilder;
use voicesub_runtime::{LOOPBACK_TOKEN_HEADER, RuntimeService};

pub struct AuthedApi<'a> {
    client: &'a reqwest::Client,
    token: &'a str,
}

impl<'a> AuthedApi<'a> {
    pub fn new(client: &'a reqwest::Client, service: &'a RuntimeService) -> Self {
        Self {
            client,
            token: service.loopback_api_token(),
        }
    }

    pub fn get(&self, url: impl AsRef<str>) -> RequestBuilder {
        self.client
            .get(url.as_ref())
            .header(LOOPBACK_TOKEN_HEADER, self.token)
    }

    #[allow(dead_code)]
    pub fn post(&self, url: impl AsRef<str>) -> RequestBuilder {
        self.client
            .post(url.as_ref())
            .header(LOOPBACK_TOKEN_HEADER, self.token)
    }

    #[allow(dead_code)]
    pub fn delete(&self, url: impl AsRef<str>) -> RequestBuilder {
        self.client
            .delete(url.as_ref())
            .header(LOOPBACK_TOKEN_HEADER, self.token)
    }
}
