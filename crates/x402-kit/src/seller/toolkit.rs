use std::fmt::Display;

use http::{HeaderMap, StatusCode};

use crate::{
    concepts::Facilitator,
    transport::{
        Base64EncodedHeader, FacilitatorPaymentRequest, FacilitatorPaymentRequestPayload,
        FacilitatorSettleFailed, FacilitatorSettleResponse, FacilitatorSettleSuccess,
        FacilitatorSupportedResponse, FacilitatorVerifyInvalid, FacilitatorVerifyResponse,
        FacilitatorVerifyValid, PaymentPayload, PaymentRequirements, PaymentRequirementsResponse,
        PaymentResponse,
    },
    types::X402Version,
};

/// Structured error response for payment processing.
#[derive(Debug, Clone)]
pub struct ErrorResponse {
    pub status: StatusCode,
    pub error: String,
    pub accepts: Vec<PaymentRequirements>,
}

impl ErrorResponse {
    pub fn into_payment_requirements_response(self) -> PaymentRequirementsResponse {
        PaymentRequirementsResponse {
            x402_version: X402Version::V1,
            error: self.error,
            accepts: self.accepts,
        }
    }

    pub fn payment_required(accepts: &[PaymentRequirements]) -> Self {
        ErrorResponse {
            status: StatusCode::PAYMENT_REQUIRED,
            error: "X-PAYMENT header is required".to_string(),
            accepts: accepts.to_owned(),
        }
    }

    pub fn invalid_payment(error: impl Display, accepts: &[PaymentRequirements]) -> Self {
        ErrorResponse {
            status: StatusCode::BAD_REQUEST,
            error: error.to_string(),
            accepts: accepts.to_owned(),
        }
    }

    pub fn payment_failed(error: impl Display, accepts: &[PaymentRequirements]) -> Self {
        ErrorResponse {
            status: StatusCode::PAYMENT_REQUIRED,
            error: error.to_string(),
            accepts: accepts.to_owned(),
        }
    }

    pub fn server_error(error: impl Display, accepts: &[PaymentRequirements]) -> Self {
        ErrorResponse {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: error.to_string(),
            accepts: accepts.to_owned(),
        }
    }
}

/// Extracts the payment payload from the raw X-Payment-Header.
pub fn extract_payment_payload(
    headers: &HeaderMap,
    payment_requirements: &[PaymentRequirements],
) -> Result<Base64EncodedHeader, ErrorResponse> {
    Ok(Base64EncodedHeader(
        headers
            .get("X-Payment")
            .ok_or(ErrorResponse::payment_required(payment_requirements))?
            .to_str()
            .map_err(|err| {
                ErrorResponse::invalid_payment(
                    format!("Failed to parse X-Payment header: {}", err),
                    payment_requirements,
                )
            })?
            .to_string(),
    ))
}

/// Updates the payment requirements with supported kinds from the facilitator.
pub async fn update_supported_kinds<F: Facilitator>(
    facilitator: &F,
    payment_requirements: Vec<PaymentRequirements>,
) -> Result<Vec<PaymentRequirements>, ErrorResponse> {
    let supported = facilitator
        .supported()
        .await
        .map_err(|err| ErrorResponse::server_error(err, &payment_requirements))?;

    Ok(filter_supported_kinds(&supported, payment_requirements))
}

/// Filters the payment requirements based on the supported kinds from the facilitator.
///
/// Returns only the payment requirements that are supported by the facilitator with updated extra fields.
pub fn filter_supported_kinds(
    supported: &FacilitatorSupportedResponse,
    payment_requirements: Vec<PaymentRequirements>,
) -> Vec<PaymentRequirements> {
    payment_requirements
        .into_iter()
        .filter_map(|mut pr| {
            supported
                .kinds
                .iter()
                .find(|kind| kind.scheme == pr.scheme && kind.network == pr.network)
                .map(|s| {
                    // Update extra field if present
                    if s.extra.is_some() {
                        pr.extra = s.extra.clone();
                    }
                    pr
                })
        })
        .collect()
}

/// Selects the appropriate payment requirements based on the provided payment payload.
pub fn select_payment_with_payload(
    payment_requirements: &[PaymentRequirements],
    x_payment_header: &Base64EncodedHeader,
) -> Result<PaymentRequirements, ErrorResponse> {
    let payment_payload = PaymentPayload::try_from(x_payment_header.clone())
        .map_err(|err| ErrorResponse::invalid_payment(err, payment_requirements))?;

    payment_requirements
        .iter()
        .find(|pr| pr.network == payment_payload.network && pr.scheme == payment_payload.scheme)
        .cloned()
        .ok_or(ErrorResponse::invalid_payment(
            "Payment payload does not match any accepted payment requirements",
            payment_requirements,
        ))
}

/// Verifies the payment using the facilitator.
pub async fn verify_payment<F: Facilitator>(
    facilitator: &F,
    x_payment_header: &Base64EncodedHeader,
    selected: &PaymentRequirements,
    payment_requirements: &[PaymentRequirements],
) -> Result<FacilitatorVerifyValid, ErrorResponse> {
    let payment_payload = x_payment_header
        .clone()
        .try_into()
        .map_err(|err| ErrorResponse::invalid_payment(err, payment_requirements))?;

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Verifying payment for scheme={}, network={}",
        selected.scheme,
        selected.network,
    );

    let request = FacilitatorPaymentRequest {
        payload: FacilitatorPaymentRequestPayload {
            payment_payload,
            payment_requirements: selected.clone(),
        },
        x_payment_header: x_payment_header.clone(),
    };

    let verify_response = facilitator
        .verify(request)
        .await
        .map_err(|err| ErrorResponse::server_error(err, payment_requirements))?;

    match verify_response {
        FacilitatorVerifyResponse::Valid(valid) => Ok(valid),
        FacilitatorVerifyResponse::Invalid(FacilitatorVerifyInvalid {
            invalid_reason,
            payer,
        }) => Err(ErrorResponse::invalid_payment(
            format!(
                "Invalid payment: reason='{invalid_reason}', payer={}",
                payer.unwrap_or("[Unknown]".to_string())
            ),
            payment_requirements,
        )),
    }
}

/// Settles the payment using the facilitator.
pub async fn settle_payment<F: Facilitator>(
    facilitator: &F,
    x_payment_header: &Base64EncodedHeader,
    selected: &PaymentRequirements,
    payment_requirements: &[PaymentRequirements],
) -> Result<FacilitatorSettleSuccess, ErrorResponse> {
    let payment_payload = x_payment_header
        .clone()
        .try_into()
        .map_err(|err| ErrorResponse::invalid_payment(err, payment_requirements))?;

    let settle_response: FacilitatorSettleResponse = facilitator
        .settle(FacilitatorPaymentRequest {
            payload: FacilitatorPaymentRequestPayload {
                payment_payload,
                payment_requirements: selected.clone(),
            },
            x_payment_header: x_payment_header.clone(),
        })
        .await
        .map_err(|err| ErrorResponse::server_error(err, payment_requirements))?;

    match settle_response {
        FacilitatorSettleResponse::Success(success) => Ok(success),
        FacilitatorSettleResponse::Failed(FacilitatorSettleFailed {
            error_reason,
            payer,
        }) => Err(ErrorResponse::payment_failed(
            format!(
                "Payment settlement failed: reason='{}', payer={}",
                error_reason,
                payer.unwrap_or("[Unknown]".to_string())
            ),
            payment_requirements,
        )),
    }
}

/// Entrypoint for processing a payment.
pub async fn process_payment<F: Facilitator>(
    facilitator: &F,
    headers: &HeaderMap,
    payment_requirements: Vec<PaymentRequirements>,
) -> Result<PaymentResponse, ErrorResponse> {
    let x_payment_header = extract_payment_payload(headers, &payment_requirements)?;
    let selected = select_payment_with_payload(&payment_requirements, &x_payment_header)?;

    let valid = verify_payment(
        facilitator,
        &x_payment_header,
        &selected,
        &payment_requirements,
    )
    .await?;

    #[cfg(feature = "tracing")]
    tracing::debug!("Payment verified for payer: {}", valid.payer);

    let settle_response = settle_payment(
        facilitator,
        &x_payment_header,
        &selected,
        &payment_requirements,
    )
    .await?;

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Payment settled: payer='{}', network='{}', transaction='{}'",
        settle_response.payer,
        settle_response.network,
        settle_response.transaction
    );

    let payer = if settle_response.payer.is_empty() && !valid.payer.is_empty() {
        valid.payer
    } else {
        settle_response.payer
    };

    Ok(PaymentResponse {
        success: true,
        transaction: settle_response.transaction,
        network: settle_response.network,
        payer,
    })
}

/// Entrypoint for processing a payment, without verification.
pub async fn process_payment_no_verify<F: Facilitator>(
    facilitator: &F,
    headers: &HeaderMap,
    payment_requirements: Vec<PaymentRequirements>,
) -> Result<PaymentResponse, ErrorResponse> {
    let updated_requirements = update_supported_kinds(facilitator, payment_requirements).await?;
    let x_payment_header = extract_payment_payload(headers, &updated_requirements)?;
    let selected = select_payment_with_payload(&updated_requirements, &x_payment_header)?;

    let settle_response = settle_payment(
        facilitator,
        &x_payment_header,
        &selected,
        &updated_requirements,
    )
    .await?;

    Ok(PaymentResponse {
        success: true,
        transaction: settle_response.transaction,
        network: settle_response.network,
        payer: settle_response.payer,
    })
}

/// Entrypoint for processing a payment, without settlement.
pub async fn process_payment_no_settle<F: Facilitator>(
    facilitator: &F,
    headers: &HeaderMap,
    payment_requirements: Vec<PaymentRequirements>,
) -> Result<FacilitatorVerifyValid, ErrorResponse> {
    let updated_requirements = update_supported_kinds(facilitator, payment_requirements).await?;
    let x_payment_header = extract_payment_payload(headers, &updated_requirements)?;
    let selected = select_payment_with_payload(&updated_requirements, &x_payment_header)?;

    let verify_response = verify_payment(
        facilitator,
        &x_payment_header,
        &selected,
        &updated_requirements,
    )
    .await?;

    Ok(verify_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_supported_kinds() {
        let supported = serde_json::from_str(
            "{
  \"kinds\": [
    {
      \"x402Version\": 1,
      \"scheme\": \"exact\",
      \"network\": \"base-sepolia\"
    },
    {
      \"x402Version\": 1,
      \"scheme\": \"exact\",
      \"network\": \"base\"
    },
    {
      \"x402Version\": 1,
      \"scheme\": \"exact\",
      \"network\": \"my-mock-network\",
      \"extra\": {
        \"mockField\": \"mockValue\"
      }
    },
    {
      \"x402Version\": 1,
      \"scheme\": \"exact\",
      \"network\": \"solana-devnet\",
      \"extra\": {
        \"feePayer\": \"2wKupLR9q6wXYppw8Gr2NvWxKBUqm4PPJKkQfoxHDBg4\"
      }
    },
    {
      \"x402Version\": 1,
      \"scheme\": \"exact\",
      \"network\": \"solana\",
      \"extra\": {
        \"feePayer\": \"2wKupLR9q6wXYppw8Gr2NvWxKBUqm4PPJKkQfoxHDBg4\"
      }
    }
  ]
}",
        )
        .unwrap();

        // EVM chains: "supported" doesn't include "extra" field, but payment requirements do
        let payment_requirements = serde_json::from_value(serde_json::json!([{
            "scheme": "exact",
            "network": "base",
            "maxAmountRequired": "100",
            "resource": "https://devnet.aimo.network/api/v1/chat/completions",
            "description": "LLM Generation endpoint",
            "mimeType": "application/json",
            "payTo": "0xD14cE79C13CE71a853eF3E8BD75969d4BDEE39c1",
            "maxTimeoutSeconds": 60,
            "asset": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            "outputSchema": {
                "input": {
                    "discoverable": true,
                    "type": "http",
                    "method": "POST"
                }
            },
            "extra": {
                "name": "USD Coin",
                "version": "2"
            }
        }]))
        .unwrap();
        // Expect the final filtered item to include the "extra" field from payment requirements
        let filtered = filter_supported_kinds(&supported, payment_requirements);
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].extra,
            Some(serde_json::json!({"name": "USD Coin", "version": "2"}))
        );

        // Solana chains: "supported" includes "extra" field, which should override payment requirements
        let payment_requirements_solana = serde_json::from_str(
            "[
    {
      \"scheme\": \"exact\",
      \"network\": \"solana\",
      \"maxAmountRequired\": \"100\",
      \"resource\": \"https://devnet.aimo.network/api/v1/chat/completions\",
      \"description\": \"LLM Generation endpoint\",
      \"mimeType\": \"application/json\",
      \"payTo\": \"Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR\",
      \"maxTimeoutSeconds\": 60,
      \"asset\": \"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v\",
      \"outputSchema\": {
        \"input\": {
          \"discoverable\": true,
          \"type\": \"http\",
          \"method\": \"POST\"
        }
      }
    }
  ]",
        )
        .unwrap();

        // Expect the final filtered item to include the "extra" field from supported kinds
        let filtered = filter_supported_kinds(&supported, payment_requirements_solana);
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].extra,
            Some(serde_json::json!({"feePayer": "2wKupLR9q6wXYppw8Gr2NvWxKBUqm4PPJKkQfoxHDBg4"}))
        );

        // Mock network: "supported" includes "extra" field, which should override payment requirements
        let payment_requirements_mock = serde_json::from_str(
            "[
    {
      \"scheme\": \"exact\",
      \"network\": \"my-mock-network\",
      \"maxAmountRequired\": \"100\",
      \"resource\": \"https://devnet.aimo.network/api/v1/chat/completions\",
      \"description\": \"LLM Generation endpoint\",
      \"mimeType\": \"application/json\",
      \"payTo\": \"0xD14cE79C13CE71a853eF3E8BD75969d4BDEE39c1\",
      \"maxTimeoutSeconds\": 60,
      \"asset\": \"0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913\",
      \"outputSchema\": {
        \"input\": {
          \"discoverable\": true,
          \"type\": \"http\",
          \"method\": \"POST\"
        }
      },
      \"extra\": {
        \"name\": \"USD Coin\",
        \"version\": \"2\"
      }
    }
  ]",
        )
        .unwrap();

        // Expect the final filtered item to include the "extra" field from supported kinds
        let filtered = filter_supported_kinds(&supported, payment_requirements_mock);
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].extra,
            Some(serde_json::json!({"mockField": "mockValue"}))
        );
    }
}
