use axum::{
    Json,
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{
    types::Base64EncodedHeader,
    v1::{
        facilitator::{Facilitator, FacilitatorSettleSuccess, FacilitatorVerifyValid},
        seller::toolkit::{
            extract_payment_payload, select_payment_with_payload, settle_payment,
            update_supported_kinds, verify_payment,
        },
        transport::{PaymentRequirements, PaymentResponse},
    },
};

#[derive(Debug, Clone)]
pub struct PaymentErrorResponse(pub super::toolkit::ErrorResponse);

impl From<super::toolkit::ErrorResponse> for PaymentErrorResponse {
    fn from(err: super::toolkit::ErrorResponse) -> Self {
        PaymentErrorResponse(err)
    }
}

impl IntoResponse for PaymentErrorResponse {
    fn into_response(self) -> Response {
        (
            self.0.status,
            Json(self.0.into_payment_requirements_response()),
        )
            .into_response()
    }
}

#[derive(Debug)]
pub struct PaymentSuccessResponse {
    pub response: Response,
    pub payment_response: PaymentResponse,
}

impl IntoResponse for PaymentSuccessResponse {
    fn into_response(self) -> Response {
        let PaymentSuccessResponse {
            mut response,
            payment_response,
        } = self;

        if let Some(header) = Base64EncodedHeader::try_from(payment_response)
            .ok()
            .and_then(|h| h.0.parse().ok())
        {
            response.headers_mut().insert("X-Payment-Response", header);
        }

        response
    }
}

pub struct PaymentHandler<F: Facilitator> {
    pub facilitator: F,
    pub payment_requirements: Vec<PaymentRequirements>,
}

/// An axum Extension extractor for proceessed payments
#[derive(Debug, Clone)]
pub enum PaymentProcessingState {
    Verified(FacilitatorVerifyValid),
    NotVerified,
    Settled(FacilitatorSettleSuccess),
}

#[bon::bon]
impl<F: Facilitator> PaymentHandler<F> {
    pub fn builder(facilitator: F) -> PaymentHandlerBuilder<F> {
        PaymentHandlerBuilder {
            facilitator,
            payment_requirements: Vec::new(),
        }
    }

    #[builder]
    pub async fn handle_payment(
        self,
        #[builder(with = || ())] no_update_supported: Option<()>,
        #[builder(with = || ())] no_verify: Option<()>,
        #[builder(with = || ())] settle_after_next: Option<()>,
        mut req: Request,
        next: Next,
    ) -> Result<PaymentSuccessResponse, PaymentErrorResponse> {
        let payment_requirements = if no_update_supported.is_none() {
            // Should update supported kinds
            update_supported_kinds(&self.facilitator, self.payment_requirements).await?
        } else {
            self.payment_requirements
        };

        let x_payment_header = extract_payment_payload(req.headers(), &payment_requirements)?;
        let selected = select_payment_with_payload(&payment_requirements, &x_payment_header)?;

        let verify = if no_verify.is_none() {
            // Should verify payment
            let valid = verify_payment(
                &self.facilitator,
                &x_payment_header,
                &selected,
                &payment_requirements,
            )
            .await?;

            #[cfg(feature = "tracing")]
            tracing::debug!("Payment verified: payer='{}'", valid.payer);

            Some(valid)
        } else {
            None
        };

        if settle_after_next.is_none() {
            // Settle before proceeding
            let settled = settle_payment(
                &self.facilitator,
                &x_payment_header,
                &selected,
                &payment_requirements,
            )
            .await?;

            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Payment settled: payer='{}', transaction='{}', network='{}'",
                settled.payer,
                settled.transaction,
                settled.network
            );

            let extension = PaymentProcessingState::Settled(settled.clone());
            req.extensions_mut().insert(extension.clone());

            #[cfg(feature = "tracing")]
            tracing::debug!("Calling next handler with extension {:?}", extension);
            let response = next.run(req).await;

            Ok(PaymentSuccessResponse {
                response,
                payment_response: settled.into(),
            })
        } else {
            // Proceed first, then settle
            let extension = verify
                .map(PaymentProcessingState::Verified)
                .unwrap_or(PaymentProcessingState::NotVerified);

            req.extensions_mut().insert(extension.clone());

            #[cfg(feature = "tracing")]
            tracing::debug!("Calling next handler with extension {:?}", extension);
            let response = next.run(req).await;

            let settled = settle_payment(
                &self.facilitator,
                &x_payment_header,
                &selected,
                &payment_requirements,
            )
            .await?;

            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Payment settled: payer='{}', transaction='{}', network='{}'",
                settled.payer,
                settled.transaction,
                settled.network
            );

            Ok(PaymentSuccessResponse {
                response,
                payment_response: settled.into(),
            })
        }
    }
}

pub struct PaymentHandlerBuilder<F: Facilitator> {
    pub facilitator: F,
    pub payment_requirements: Vec<PaymentRequirements>,
}

impl<F: Facilitator> PaymentHandlerBuilder<F> {
    pub fn add_payment(mut self, payment_requirements: impl Into<PaymentRequirements>) -> Self {
        self.payment_requirements.push(payment_requirements.into());
        self
    }

    pub fn build(self) -> PaymentHandler<F> {
        PaymentHandler {
            facilitator: self.facilitator,
            payment_requirements: self.payment_requirements,
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::address;
    use axum::middleware::from_fn;
    use tower::ServiceBuilder;
    use url_macro::url;

    use crate::{
        core::Resource, networks::evm::assets::UsdcBase, schemes::exact_evm::ExactEvm,
        v1::facilitator_client::RemoteFacilitatorClient,
    };

    use super::*;

    async fn middleware_fn(
        req: Request,
        next: Next,
    ) -> Result<PaymentSuccessResponse, PaymentErrorResponse> {
        PaymentHandler::builder(RemoteFacilitatorClient::from_url(url!(
            "https://facilitator.example.com"
        )))
        .add_payment(
            ExactEvm::builder()
                .asset(UsdcBase)
                .amount(1000_000)
                .pay_to(address!("0x17d2e11d0405fa8d0ad2dca6409c499c0132c017"))
                .resource(
                    Resource::builder()
                        .url(url!("https://my-site.com/api"))
                        .description("")
                        .mime_type("")
                        .build(),
                )
                .build()
                .v1(),
        )
        .build()
        .handle_payment()
        .no_verify()
        .no_update_supported()
        .settle_after_next()
        .req(req)
        .next(next)
        .call()
        .await
    }

    #[test]
    fn test_build_axum_middleware() {
        let _ = ServiceBuilder::new().layer(from_fn::<_, (Request,)>(middleware_fn));
    }
}
