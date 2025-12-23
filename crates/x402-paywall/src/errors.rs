use http::{HeaderName, HeaderValue, StatusCode};
use x402_kit::{transport::PaymentRequired, types::Base64EncodedHeader};

/// Represents an error response from the paywall.
#[derive(Debug, Clone)]
pub struct ErrorResponse {
    pub status: StatusCode,
    pub header: ErrorResponseHeader,
    pub body: PaymentRequired,
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
                    .map(|v| (HeaderName::from_static("PAYMENT-REQUIRED"), v))
            }
            ErrorResponseHeader::PaymentResponse(Base64EncodedHeader(s)) => {
                HeaderValue::from_str(&s)
                    .ok()
                    .map(|v| (HeaderName::from_static("PAYMENT-RESPONSE"), v))
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
