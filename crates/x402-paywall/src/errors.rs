use http::{HeaderName, HeaderValue, StatusCode};
use x402_core::{transport::PaymentRequired, types::Base64EncodedHeader};

/// Represents an error response from the paywall.
#[derive(Debug, Clone)]
pub struct ErrorResponse {
    pub status: StatusCode,
    pub header: ErrorResponseHeader,
    /// The body of the error response.
    ///
    /// Body is Boxed to reduce size of the struct.
    pub body: Box<PaymentRequired>,
}

/// Represents the type of error header to include in a paywall error response.
#[derive(Debug, Clone)]
pub enum ErrorResponseHeader {
    PaymentRequired(Base64EncodedHeader),
    PaymentResponse(Base64EncodedHeader),
}

impl ErrorResponseHeader {
    /// Get the header value to include in the response.
    ///
    /// Returns `None` if the header value could not be created.
    pub fn header_value(self) -> Option<(HeaderName, HeaderValue)> {
        match self {
            ErrorResponseHeader::PaymentRequired(Base64EncodedHeader(s)) => {
                HeaderValue::from_str(&s)
                    .ok()
                    .map(|v| (HeaderName::from_static("payment-required"), v))
            }
            ErrorResponseHeader::PaymentResponse(Base64EncodedHeader(s)) => {
                HeaderValue::from_str(&s)
                    .ok()
                    .map(|v| (HeaderName::from_static("payment-response"), v))
            }
        }
    }
}

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let mut response = (self.status, axum::extract::Json(self.body)).into_response();
        if let Some((name, val)) = self.header.header_value() {
            response.headers_mut().insert(name, val);
        }
        response
    }
}
