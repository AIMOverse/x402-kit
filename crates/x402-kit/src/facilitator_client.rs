use http::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    concepts::Facilitator,
    transport::{
        FacilitatorPaymentRequest, FacilitatorSettleFailed, FacilitatorSettleResponse,
        FacilitatorSettleSuccess, FacilitatorSupportedResponse, FacilitatorVerifyInvalid,
        FacilitatorVerifyResponse, FacilitatorVerifyValid, PaymentPayload, PaymentRequirements,
    },
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
///
/// # Examples
///
/// Create a client with the default payloads and a shared API key header:
///
/// ```rust
/// # use http::{HeaderName, HeaderValue};
/// # use url::Url;
/// # use x402_kit::facilitator_client::DefaultRemoteFacilitatorClient;
/// let client = DefaultRemoteFacilitatorClient::from_url(
///     Url::parse("https://facilitator.example.com").unwrap(),
/// )
/// .header(
///     &HeaderName::from_static("x-api-key"),
///     &HeaderValue::from_static("super-secret"),
/// );
/// ```
///
/// Swap only the verify response type while keeping every other parameter:
///
/// ```rust
/// # use serde::Deserialize;
/// # use x402_kit::facilitator_client::{DefaultRemoteFacilitatorClient, IntoVerifyResponse};
/// # use x402_kit::transport::{
/// #     FacilitatorVerifyInvalid,
/// #     FacilitatorVerifyResponse,
/// #     FacilitatorVerifyValid,
/// # };
/// #[derive(Deserialize)]
/// struct CustomVerifyResponse {
///     ok: bool,
/// }
///
/// impl IntoVerifyResponse for CustomVerifyResponse {
///     fn into_verify_response(self) -> FacilitatorVerifyResponse {
///         if self.ok {
///             FacilitatorVerifyResponse::valid(FacilitatorVerifyValid {
///                 payer: String::new(),
///             })
///         } else {
///             FacilitatorVerifyResponse::invalid(FacilitatorVerifyInvalid {
///                 invalid_reason: "not ok".into(),
///                 payer: None,
///             })
///         }
///     }
/// }
///
/// # use url::Url;
/// let client = DefaultRemoteFacilitatorClient::from_url(Url::parse("https://facilitator.example.com").unwrap())
///     .with_verify_response_type::<CustomVerifyResponse>();
/// ```
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
    pub supported_headers: HeaderMap,
    pub verify_headers: HeaderMap,
    pub settle_headers: HeaderMap,
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
    /// Creates a new client rooted at the provided `base_url` with a default `reqwest::Client` and
    /// no headers configured.
    ///
    /// Use the builder-style helpers such as [`RemoteFacilitatorClient::header`] and
    /// [`RemoteFacilitatorClient::with_verify_request_type`] to refine the client before issuing
    /// any HTTP calls.
    pub fn new_from_url(base_url: Url) -> Self {
        RemoteFacilitatorClient {
            base_url,
            client: reqwest::Client::new(),
            supported_headers: HeaderMap::new(),
            verify_headers: HeaderMap::new(),
            settle_headers: HeaderMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Overrides the request payload type used when calling `/verify` while keeping every other
    /// type parameter intact. This is useful when the remote facilitator expects a different JSON
    /// layout than the default [`DefaultPaymentRequest`].
    pub fn with_verify_request_type<NewVReq>(
        self,
    ) -> RemoteFacilitatorClient<NewVReq, VRes, SReq, SRes>
    where
        NewVReq: From<FacilitatorPaymentRequest> + Serialize,
    {
        RemoteFacilitatorClient {
            base_url: self.base_url,
            client: self.client,
            supported_headers: self.supported_headers,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Overrides the response payload type returned by `/verify`. The provided `NewVRes` just needs
    /// to deserialize and implement [`IntoVerifyResponse`] so the library can convert it back into a
    /// [`FacilitatorVerifyResponse`].
    pub fn with_verify_response_type<NewVRes>(
        self,
    ) -> RemoteFacilitatorClient<VReq, NewVRes, SReq, SRes>
    where
        NewVRes: IntoVerifyResponse + for<'de> Deserialize<'de>,
    {
        RemoteFacilitatorClient {
            supported_headers: self.supported_headers,
            base_url: self.base_url,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            client: self.client,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Overrides the request payload type sent to `/settle` while keeping every other type
    /// parameter unchanged.
    pub fn with_settle_request_type<NewSReq>(
        self,
    ) -> RemoteFacilitatorClient<VReq, VRes, NewSReq, SRes>
    where
        NewSReq: From<FacilitatorPaymentRequest> + Serialize,
    {
        RemoteFacilitatorClient {
            supported_headers: self.supported_headers,
            base_url: self.base_url,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            client: self.client,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Overrides the response payload type returned by `/settle`. The type must deserialize and
    /// implement [`IntoSettleResponse`].
    pub fn with_settle_response_type<NewSRes>(
        self,
    ) -> RemoteFacilitatorClient<VReq, VRes, SReq, NewSRes>
    where
        NewSRes: IntoSettleResponse + for<'de> Deserialize<'de>,
    {
        RemoteFacilitatorClient {
            supported_headers: self.supported_headers,
            base_url: self.base_url,
            verify_headers: self.verify_headers,
            settle_headers: self.settle_headers,
            client: self.client,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Adds a header that will be sent to all three endpoints (`/supported`, `/verify`, `/settle`).
    ///
    /// ```rust
    /// # use url::Url;
    /// # use http::{HeaderName, HeaderValue};
    /// # use x402_kit::facilitator_client::DefaultRemoteFacilitatorClient;
    /// let client = DefaultRemoteFacilitatorClient::from_url(Url::parse("https://example.com").unwrap())
    ///     .header(&HeaderName::from_static("x-api-key"), &HeaderValue::from_static("secret"));
    /// ```
    ///
    /// Prefer this helper when an auth token or trace header should accompany every request.
    pub fn header(mut self, key: &HeaderName, value: &HeaderValue) -> Self {
        self.supported_headers.insert(key, value.to_owned());
        self.verify_headers.insert(key, value.to_owned());
        self.settle_headers.insert(key, value.to_owned());
        self
    }

    /// Adds a header that is only used for the `/supported` endpoint. Useful for flags or tokens
    /// that differ from the `/verify` or `/settle` requirements.
    pub fn supported_header(mut self, key: &HeaderName, value: &HeaderValue) -> Self {
        self.supported_headers.insert(key, value.to_owned());
        self
    }

    /// Adds a header that is only used for the `/verify` endpoint.
    pub fn verify_header(mut self, key: &HeaderName, value: &HeaderValue) -> Self {
        self.verify_headers.insert(key, value.to_owned());
        self
    }

    /// Adds a header that is only used for the `/settle` endpoint.
    pub fn settle_header(mut self, key: &HeaderName, value: &HeaderValue) -> Self {
        self.settle_headers.insert(key, value.to_owned());
        self
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
    /// Convenience constructor for the default request/response configuration.
    ///
    /// Use the builder-style helpers such as [`RemoteFacilitatorClient::header`] and
    /// [`RemoteFacilitatorClient::with_verify_request_type`] to refine the client before issuing
    /// any HTTP calls.
    pub fn from_url(base_url: Url) -> Self {
        RemoteFacilitatorClient::new_from_url(base_url)
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

    async fn supported(&self) -> Result<FacilitatorSupportedResponse, Self::Error> {
        let supported = self
            .client
            .get(self.base_url.join("supported")?)
            .headers(self.supported_headers.clone())
            .send()
            .await?
            .json()
            .await?;

        Ok(supported)
    }

    async fn verify(
        &self,
        request: FacilitatorPaymentRequest,
    ) -> Result<FacilitatorVerifyResponse, Self::Error> {
        let result = self
            .client
            .post(self.base_url.join("verify")?)
            .headers(self.verify_headers.clone())
            .json(&VReq::from(request))
            .send()
            .await?
            .json::<VRes>()
            .await?;

        Ok(result.into_verify_response())
    }

    async fn settle(
        &self,
        request: FacilitatorPaymentRequest,
    ) -> Result<FacilitatorSettleResponse, Self::Error> {
        let result = self
            .client
            .post(self.base_url.join("settle")?)
            .headers(self.settle_headers.clone())
            .json(&SReq::from(request))
            .send()
            .await?
            .json::<SRes>()
            .await?;

        Ok(result.into_settle_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{HeaderName, HeaderValue};
    use url::Url;

    fn test_client() -> DefaultRemoteFacilitatorClient {
        DefaultRemoteFacilitatorClient::from_url(Url::parse("https://example.com/").unwrap())
    }

    #[test]
    fn header_sets_all_maps() {
        let header_name = HeaderName::from_static("x-shared-header");
        let header_value = HeaderValue::from_static("shared-value");

        let client = test_client().header(&header_name, &header_value);

        for map in [
            &client.supported_headers,
            &client.verify_headers,
            &client.settle_headers,
        ] {
            let stored = map.get(&header_name).expect("header missing");
            assert_eq!(stored, &header_value);
        }
    }

    #[test]
    fn supported_header_only_affects_supported_map() {
        let header_name = HeaderName::from_static("x-supported-only");
        let header_value = HeaderValue::from_static("supported-value");

        let client = test_client().supported_header(&header_name, &header_value);

        assert_eq!(
            client.supported_headers.get(&header_name),
            Some(&header_value)
        );
        assert!(client.verify_headers.get(&header_name).is_none());
        assert!(client.settle_headers.get(&header_name).is_none());
    }

    #[test]
    fn verify_header_only_affects_verify_map() {
        let header_name = HeaderName::from_static("x-verify-only");
        let header_value = HeaderValue::from_static("verify-value");

        let client = test_client().verify_header(&header_name, &header_value);

        assert_eq!(client.verify_headers.get(&header_name), Some(&header_value));
        assert!(client.supported_headers.get(&header_name).is_none());
        assert!(client.settle_headers.get(&header_name).is_none());
    }

    #[test]
    fn settle_header_only_affects_settle_map() {
        let header_name = HeaderName::from_static("x-settle-only");
        let header_value = HeaderValue::from_static("settle-value");

        let client = test_client().settle_header(&header_name, &header_value);

        assert_eq!(client.settle_headers.get(&header_name), Some(&header_value));
        assert!(client.supported_headers.get(&header_name).is_none());
        assert!(client.verify_headers.get(&header_name).is_none());
    }
}
