use bon::Builder;
use url::Url;

use crate::concepts::Facilitator;

#[derive(Builder, Debug, Clone)]
pub struct RemoteFacilitatorClient {
    pub base_url: Url,
    pub client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteFacilitatorClientError {
    #[error("URL parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("HTTP request error: {0}")]
    HttpRequestError(#[from] reqwest::Error),
    #[error("Serialization/Deserialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

impl Facilitator for RemoteFacilitatorClient {
    type Error = RemoteFacilitatorClientError;

    async fn supported(
        &self,
    ) -> Result<crate::transports::FacilitatorSupportedResponse, Self::Error> {
        let supported = self
            .client
            .get(self.base_url.join("supported")?)
            .send()
            .await?
            .json()
            .await?;

        Ok(supported)
    }

    async fn verify(
        &self,
        request: &crate::transports::FacilitatorPaymentRequest,
    ) -> Result<crate::transports::FacilitatorVerifyResponse, Self::Error> {
        let result = self
            .client
            .post(self.base_url.join("verify")?)
            .json(request)
            .send()
            .await?
            .json()
            .await?;
        Ok(result)
    }

    async fn settle(
        &self,
        request: &crate::transports::FacilitatorPaymentRequest,
    ) -> Result<crate::transports::FacilitatorSettleResponse, Self::Error> {
        let result = self
            .client
            .post(self.base_url.join("settle")?)
            .json(request)
            .send()
            .await?
            .json()
            .await?;

        Ok(result)
    }
}
