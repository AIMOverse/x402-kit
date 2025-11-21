use std::fmt::Display;

use crate::{
    concepts::Facilitator,
    transports::{
        Base64EncodedHeader, FacilitatorPaymentRequest, FacilitatorSettleResponse,
        FacilitatorVerifyResponse, PaymentPayload, PaymentRequirements,
        PaymentRequirementsResponse, PaymentResponse,
    },
    types::X402Version,
};

/// Structured error response for payment processing.
#[derive(Debug, Clone)]
pub struct ErrorResponse {
    pub status: u16,
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

    pub fn payment_required(accepts: &Vec<PaymentRequirements>) -> Self {
        ErrorResponse {
            status: 402,
            error: "Payment required to access this resource".to_string(),
            accepts: accepts.clone(),
        }
    }

    pub fn invalid_payment(error: impl Display, accepts: &Vec<PaymentRequirements>) -> Self {
        ErrorResponse {
            status: 400,
            error: error.to_string(),
            accepts: accepts.clone(),
        }
    }

    pub fn payment_failed(error: impl Display, accepts: &Vec<PaymentRequirements>) -> Self {
        ErrorResponse {
            status: 402,
            error: error.to_string(),
            accepts: accepts.clone(),
        }
    }

    pub fn server_error(error: impl Display, accepts: &Vec<PaymentRequirements>) -> Self {
        ErrorResponse {
            status: 402,
            error: error.to_string(),
            accepts: accepts.clone(),
        }
    }
}

/// Extracts the payment payload from the raw X-Payment-Header.
pub fn extract_payment_payload(
    raw_x_payment_header: Option<&str>,
    payment_requirements: &Vec<PaymentRequirements>,
) -> Result<Base64EncodedHeader, ErrorResponse> {
    Ok(Base64EncodedHeader(
        raw_x_payment_header
            .ok_or(ErrorResponse::payment_required(payment_requirements))?
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

    let filtered: Vec<PaymentRequirements> = payment_requirements
        .into_iter()
        .filter_map(|mut pr| {
            supported
                .kinds
                .iter()
                .find(|kind| kind.scheme == pr.scheme && kind.network == pr.network)
                .and_then(|s| {
                    pr.extra = s.extra.clone();
                    Some(pr)
                })
        })
        .collect();
    Ok(filtered)
}

/// Selects the appropriate payment requirements based on the provided payment payload.
pub fn select_payment_with_payload(
    payment_requirements: &Vec<PaymentRequirements>,
    x_payment_header: &Base64EncodedHeader,
) -> Result<PaymentRequirements, ErrorResponse> {
    let payment_payload = PaymentPayload::try_from(x_payment_header.clone())
        .map_err(|err| ErrorResponse::invalid_payment(err, &payment_requirements))?;

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
    payment_requirements: &Vec<PaymentRequirements>,
) -> Result<FacilitatorVerifyResponse, ErrorResponse> {
    let payment_payload = x_payment_header
        .clone()
        .try_into()
        .map_err(|err| ErrorResponse::invalid_payment(err, &payment_requirements))?;

    let verify_response = facilitator
        .verify(&FacilitatorPaymentRequest {
            payment_header: Some(x_payment_header.clone()),
            payment_requirements: selected.clone(),
            payment_payload,
        })
        .await
        .map_err(|err| ErrorResponse::server_error(err, &payment_requirements))?;

    if verify_response.is_valid {
        Ok(verify_response)
    } else {
        Err(ErrorResponse::invalid_payment(
            verify_response
                .invalid_reason
                .unwrap_or("Unknown reason".to_string()),
            &payment_requirements,
        ))
    }
}

/// Settles the payment using the facilitator.
pub async fn settle_payment<F: Facilitator>(
    facilitator: &F,
    x_payment_header: &Base64EncodedHeader,
    selected: &PaymentRequirements,
    payment_requirements: &Vec<PaymentRequirements>,
) -> Result<FacilitatorSettleResponse, ErrorResponse> {
    let payment_payload = x_payment_header
        .clone()
        .try_into()
        .map_err(|err| ErrorResponse::invalid_payment(err, &payment_requirements))?;

    let settle_response = facilitator
        .settle(&FacilitatorPaymentRequest {
            payment_header: Some(x_payment_header.clone()),
            payment_requirements: selected.clone(),
            payment_payload,
        })
        .await
        .map_err(|err| ErrorResponse::server_error(err, &payment_requirements))?;

    if settle_response.success {
        Ok(settle_response)
    } else {
        Err(ErrorResponse::payment_failed(
            settle_response
                .error_reason
                .unwrap_or("Unknown reason".to_string()),
            &payment_requirements,
        ))
    }
}

/// Entrypoint for processing a payment.
pub async fn process_payment<F: Facilitator>(
    facilitator: &F,
    raw_x_payment_header: Option<&str>,
    payment_requirements: Vec<PaymentRequirements>,
) -> Result<PaymentResponse, ErrorResponse> {
    let updated_requirements = update_supported_kinds(facilitator, payment_requirements).await?;
    let x_payment_header = extract_payment_payload(raw_x_payment_header, &updated_requirements)?;
    let selected = select_payment_with_payload(&updated_requirements, &x_payment_header)?;

    verify_payment(
        facilitator,
        &x_payment_header,
        &selected,
        &updated_requirements,
    )
    .await?;

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

/// Entrypoint for processing a payment, without verification.
pub async fn process_payment_no_verify<F: Facilitator>(
    facilitator: &F,
    raw_x_payment_header: Option<&str>,
    payment_requirements: Vec<PaymentRequirements>,
) -> Result<PaymentResponse, ErrorResponse> {
    let updated_requirements = update_supported_kinds(facilitator, payment_requirements).await?;
    let x_payment_header = extract_payment_payload(raw_x_payment_header, &updated_requirements)?;
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
    raw_x_payment_header: Option<&str>,
    payment_requirements: Vec<PaymentRequirements>,
) -> Result<FacilitatorVerifyResponse, ErrorResponse> {
    let updated_requirements = update_supported_kinds(facilitator, payment_requirements).await?;
    let x_payment_header = extract_payment_payload(raw_x_payment_header, &updated_requirements)?;
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
