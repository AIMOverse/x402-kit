use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    concepts::Facilitator,
    transports::{FacilitatorPaymentRequest, FacilitatorSettleResponse, FacilitatorVerifyResponse},
};

/// A remote facilitator client that communicates over HTTP.
///
/// You can customize the request and response types for verification and settlement
///
/// # Type Parameters
///
/// - `VReq`: The request type for verification, must be convertible from `FacilitatorPaymentRequest` and serializable.
/// - `VRes`: The response type for verification, must be convertible into `FacilitatorVerifyResponse` and deserializable.
/// - `SReq`: The request type for settlement, must be convertible from `FacilitatorPaymentRequest` and serializable.
/// - `SRes`: The response type for settlement, must be convertible into `FacilitatorSettleResponse` and deserializable.
#[derive(Debug, Clone)]
pub struct RemoteFacilitatorClient<VReq, VRes, SReq, SRes>
where
    VReq: From<FacilitatorPaymentRequest> + Serialize,
    VRes: Into<FacilitatorVerifyResponse> + for<'de> Deserialize<'de>,
    SReq: From<FacilitatorPaymentRequest> + Serialize,
    SRes: Into<FacilitatorSettleResponse> + for<'de> Deserialize<'de>,
{
    pub base_url: Url,
    pub client: reqwest::Client,
    pub _phantom: std::marker::PhantomData<(VReq, VRes, SReq, SRes)>,
}

impl<VReq, VRes, SReq, SRes> RemoteFacilitatorClient<VReq, VRes, SReq, SRes>
where
    VReq: From<FacilitatorPaymentRequest> + Serialize,
    VRes: Into<FacilitatorVerifyResponse> + for<'de> Deserialize<'de>,
    SReq: From<FacilitatorPaymentRequest> + Serialize,
    SRes: Into<FacilitatorSettleResponse> + for<'de> Deserialize<'de>,
{
    pub fn new(base_url: Url) -> Self {
        RemoteFacilitatorClient {
            base_url,
            client: reqwest::Client::new(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl
    RemoteFacilitatorClient<
        FacilitatorPaymentRequest,
        FacilitatorVerifyResponse,
        FacilitatorPaymentRequest,
        FacilitatorSettleResponse,
    >
{
    pub fn new_default(base_url: Url) -> Self {
        RemoteFacilitatorClient::new(base_url)
    }
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

impl<VReq, VRes, SReq, SRes> Facilitator for RemoteFacilitatorClient<VReq, VRes, SReq, SRes>
where
    VReq: From<FacilitatorPaymentRequest> + Serialize,
    VRes: Into<FacilitatorVerifyResponse> + for<'de> Deserialize<'de>,
    SReq: From<FacilitatorPaymentRequest> + Serialize,
    SRes: Into<FacilitatorSettleResponse> + for<'de> Deserialize<'de>,
{
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
        request: crate::transports::FacilitatorPaymentRequest,
    ) -> Result<crate::transports::FacilitatorVerifyResponse, Self::Error> {
        let result = self
            .client
            .post(self.base_url.join("verify")?)
            .json(&VReq::from(request))
            .send()
            .await?
            .json::<VRes>()
            .await?;

        Ok(result.into())
    }

    async fn settle(
        &self,
        request: crate::transports::FacilitatorPaymentRequest,
    ) -> Result<crate::transports::FacilitatorSettleResponse, Self::Error> {
        let result = self
            .client
            .post(self.base_url.join("settle")?)
            .json(&SReq::from(request))
            .send()
            .await?
            .json::<SRes>()
            .await?;

        Ok(result.into())
    }
}
