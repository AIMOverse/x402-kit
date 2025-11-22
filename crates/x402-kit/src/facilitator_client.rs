use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    concepts::Facilitator,
    transports::{
        FacilitatorPaymentRequest, FacilitatorSettleFailed, FacilitatorSettleResponse,
        FacilitatorSettleSuccess, FacilitatorVerifyInvalid, FacilitatorVerifyResponse,
        FacilitatorVerifyValid, PaymentPayload, PaymentRequirements,
    },
    types::Record,
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
    VRes: IntoVerifyResponse + for<'de> Deserialize<'de>,
    SReq: From<FacilitatorPaymentRequest> + Serialize,
    SRes: IntoSettleResponse + for<'de> Deserialize<'de>,
{
    pub base_url: Url,
    pub client: reqwest::Client,
    pub verify_headers: Record<String>,
    pub settle_headers: Record<String>,
    pub _phantom: std::marker::PhantomData<(VReq, VRes, SReq, SRes)>,
}

pub trait IntoVerifyResponse {
    fn into_verify_response(self) -> FacilitatorVerifyResponse;
}

pub trait IntoSettleResponse {
    fn into_settle_response(self) -> FacilitatorSettleResponse;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultPaymentRequest {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultVerifyResponse {
    pub is_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    pub payer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultSettleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
    pub payer: Option<String>,
    pub transaction: Option<String>,
    pub network: Option<String>,
}

impl From<FacilitatorPaymentRequest> for DefaultPaymentRequest {
    fn from(request: FacilitatorPaymentRequest) -> Self {
        DefaultPaymentRequest {
            payment_payload: request.payload.payment_payload,
            payment_requirements: request.payload.payment_requirements,
        }
    }
}

impl IntoVerifyResponse for DefaultVerifyResponse {
    fn into_verify_response(self) -> FacilitatorVerifyResponse {
        if self.is_valid {
            FacilitatorVerifyResponse::valid(FacilitatorVerifyValid {
                payer: self.payer.unwrap_or_default(),
            })
        } else {
            FacilitatorVerifyResponse::invalid(FacilitatorVerifyInvalid {
                invalid_reason: self.invalid_reason.unwrap_or_default(),
                payer: self.payer,
            })
        }
    }
}

impl IntoSettleResponse for DefaultSettleResponse {
    fn into_settle_response(self) -> FacilitatorSettleResponse {
        if self.success {
            FacilitatorSettleResponse::success(FacilitatorSettleSuccess {
                payer: self.payer.unwrap_or_default(),
                transaction: self.transaction.unwrap_or_default(),
                network: self.network.unwrap_or_default(),
            })
        } else {
            FacilitatorSettleResponse::failed(FacilitatorSettleFailed {
                error_reason: self.error_reason.unwrap_or_default(),
                payer: self.payer,
            })
        }
    }
}

/// A type alias for a RemoteFacilitatorClient using the default request and response types.
pub type DefaultRemoteFacilitatorClient = RemoteFacilitatorClient<
    DefaultPaymentRequest,
    DefaultVerifyResponse,
    DefaultPaymentRequest,
    DefaultSettleResponse,
>;

impl<VReq, VRes, SReq, SRes> RemoteFacilitatorClient<VReq, VRes, SReq, SRes>
where
    VReq: From<FacilitatorPaymentRequest> + Serialize,
    VRes: IntoVerifyResponse + for<'de> Deserialize<'de>,
    SReq: From<FacilitatorPaymentRequest> + Serialize,
    SRes: IntoSettleResponse + for<'de> Deserialize<'de>,
{
    pub fn new(base_url: Url) -> Self {
        RemoteFacilitatorClient {
            base_url,
            client: reqwest::Client::new(),
            verify_headers: Record::new(),
            settle_headers: Record::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_verify_request_type<NewVReq>(
        self,
    ) -> RemoteFacilitatorClient<NewVReq, VRes, SReq, SRes>
    where
        NewVReq: From<FacilitatorPaymentRequest> + Serialize,
    {
        RemoteFacilitatorClient {
            base_url: self.base_url,
            client: self.client,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_verify_response_type<NewVRes>(
        self,
    ) -> RemoteFacilitatorClient<VReq, NewVRes, SReq, SRes>
    where
        NewVRes: IntoVerifyResponse + for<'de> Deserialize<'de>,
    {
        RemoteFacilitatorClient {
            base_url: self.base_url,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            client: self.client,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_settle_request_type<NewSReq>(
        self,
    ) -> RemoteFacilitatorClient<VReq, VRes, NewSReq, SRes>
    where
        NewSReq: From<FacilitatorPaymentRequest> + Serialize,
    {
        RemoteFacilitatorClient {
            base_url: self.base_url,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            client: self.client,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_settle_response_type<NewSRes>(
        self,
    ) -> RemoteFacilitatorClient<VReq, VRes, SReq, NewSRes>
    where
        NewSRes: IntoSettleResponse + for<'de> Deserialize<'de>,
    {
        RemoteFacilitatorClient {
            base_url: self.base_url,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            client: self.client,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl
    RemoteFacilitatorClient<
        DefaultPaymentRequest,
        DefaultVerifyResponse,
        DefaultPaymentRequest,
        DefaultSettleResponse,
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
    VRes: IntoVerifyResponse + for<'de> Deserialize<'de>,
    SReq: From<FacilitatorPaymentRequest> + Serialize,
    SRes: IntoSettleResponse + for<'de> Deserialize<'de>,
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

        Ok(result.into_verify_response())
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

        Ok(result.into_settle_response())
    }
}
