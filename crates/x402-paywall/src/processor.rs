use http::{HeaderValue, Request, Response};
use x402_kit::{
    facilitator::{
        Facilitator, PaymentRequest, SettleResult, SettleSuccess, VerifyResult, VerifyValid,
    },
    transport::{PaymentPayload, PaymentRequirements, SettlementResponse},
    types::{Base64EncodedHeader, Extension, Record},
};

use crate::{errors::ErrorResponse, paywall::PayWall};

/// The state of a payment processed by the paywall when accessing the resource handler.
#[derive(Debug, Clone)]
pub struct PaymentState {
    /// Verification result, if verification was performed.
    pub verified: Option<VerifyValid>,
    /// Settlement result, if settlement was performed.
    pub settled: Option<SettleSuccess>,
    /// All extensions info provided by the paywall.
    pub required_extensions: Record<Extension>,
    /// All extensions info provided by the signer.
    pub payload_extensions: Record<Extension>,
}

/// Payment processing state before running the resource handler.
pub struct RequestProcessor<'pw, F: Facilitator, Req> {
    pub paywall: &'pw PayWall<F>,
    pub request: Request<Req>,
    pub payload: PaymentPayload,
    pub selected: PaymentRequirements,
    pub payment_state: PaymentState,
}

impl<'pw, F: Facilitator, Req> RequestProcessor<'pw, F, Req> {
    /// Verify the payment with the facilitator.
    ///
    /// `self.payment_state.verified` will be populated on success.
    pub async fn verify(mut self) -> Result<Self, ErrorResponse> {
        let response = self
            .paywall
            .facilitator
            .verify(PaymentRequest {
                payment_payload: self.payload.clone(),
                payment_requirements: self.selected.clone(),
            })
            .await
            .map_err(|err| {
                self.paywall
                    .server_error(format!("Failed to verify payment: {err}"))
            })?;

        let valid = match response {
            VerifyResult::Valid(v) => v,
            VerifyResult::Invalid(iv) => {
                return Err(self.paywall.payment_failed(iv.invalid_reason));
            }
        };

        #[cfg(feature = "tracing")]
        tracing::debug!("Payment verified: payer='{}'", valid.payer);

        self.payment_state.verified = Some(valid);

        Ok(self)
    }

    /// Settle the payment with the facilitator.
    ///
    /// `self.payment_state.settled` will be populated on success.
    pub async fn settle(mut self) -> Result<Self, ErrorResponse> {
        let settlement = self
            .paywall
            .facilitator
            .settle(PaymentRequest {
                payment_payload: self.payload.clone(),
                payment_requirements: self.selected.clone(),
            })
            .await
            .map_err(|err| {
                self.paywall
                    .server_error(format!("Failed to settle payment: {err}"))
            })?;

        let settled = match settlement {
            SettleResult::Success(s) => s,
            SettleResult::Failed(f) => {
                return Err(self.paywall.payment_failed(f.error_reason));
            }
        };

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Payment settled: payer='{}', transaction='{}', network='{}'",
            settled.payer,
            settled.transaction,
            settled.network
        );

        self.payment_state.settled = Some(settled);

        Ok(self)
    }

    /// Run the resource handler with the payment state attached to the request extensions.
    ///
    /// After running the handler, returns a [`PayWallResponse`] for further processing.
    pub async fn run_handler<Fun, Fut, Res>(
        mut self,
        handler: Fun,
    ) -> Result<ResponseProcessor<'pw, F, Res>, ErrorResponse>
    where
        Fun: FnOnce(Request<Req>) -> Fut,
        Fut: Future<Output = Response<Res>>,
    {
        self.request
            .extensions_mut()
            .insert(self.payment_state.clone());

        let response = handler(self.request).await;
        Ok(ResponseProcessor {
            paywall: self.paywall,
            response,
            payload: self.payload,
            selected: self.selected,
            payment_state: self.payment_state,
        })
    }
}

/// Payment processing state after running the resource handler.
pub struct ResponseProcessor<'pw, F: Facilitator, Res> {
    pub paywall: &'pw PayWall<F>,
    pub response: Response<Res>,
    pub payload: PaymentPayload,
    pub selected: PaymentRequirements,
    pub payment_state: PaymentState,
}

impl<'pw, F: Facilitator, Res> ResponseProcessor<'pw, F, Res> {
    /// Settle the payment with the facilitator after running the resource handler.
    ///
    /// After settlement, `self.payment_state.settled` will be populated on success.
    pub async fn settle(mut self) -> Result<Self, ErrorResponse> {
        // Settle payment with facilitator
        let settlement = self
            .paywall
            .facilitator
            .settle(PaymentRequest {
                payment_payload: self.payload.clone(),
                payment_requirements: self.selected.clone(),
            })
            .await
            .map_err(|err| {
                self.paywall
                    .server_error(format!("Failed to settle payment: {err}"))
            })?;

        let settled = match settlement {
            SettleResult::Success(s) => s,
            SettleResult::Failed(f) => {
                return Err(self.paywall.payment_failed(f.error_reason));
            }
        };

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Payment settled: payer='{}', transaction='{}', network='{}'",
            settled.payer,
            settled.transaction,
            settled.network
        );

        self.payment_state.settled = Some(settled);
        Ok(self)
    }

    /// Conditionally settle the payment based on the provided prediction function.
    ///
    /// After settlement, `self.payment_state.settled` will be populated on success.
    pub async fn settle_on(
        self,
        predicate: impl Fn(&Response<Res>) -> bool,
    ) -> Result<Self, ErrorResponse> {
        if predicate(&self.response) {
            self.settle().await
        } else {
            Ok(self)
        }
    }

    /// Settle the payment if the response status is a success (2xx).
    ///
    /// After settlement, `self.payment_state.settled` will be populated on success.
    pub async fn settle_on_success(self) -> Result<Self, ErrorResponse> {
        self.settle_on(|resp| resp.status().is_success()).await
    }

    /// Generate the final response, including the `PAYMENT-RESPONSE` header if settled.
    pub fn response(self) -> Response<Res> {
        let mut response = self.response;

        if let Some(settled) = &self.payment_state.settled {
            let settlement_response = SettlementResponse {
                success: true,
                payer: settled.payer.clone(),
                transaction: settled.transaction.clone(),
                network: settled.network.clone(),
            };

            let header = Base64EncodedHeader::try_from(settlement_response)
                .inspect_err(|err| {
                    tracing::warn!("Failed to encode PAYMENT-RESPONSE header: {err}; skipping")
                })
                .ok()
                .and_then(|h| {
                    HeaderValue::from_str(&h.0)
                        .inspect_err(|err| {
                            tracing::warn!(
                                "Failed to encode PAYMENT-RESPONSE header: {err}; skipping"
                            )
                        })
                        .ok()
                });

            if let Some(header) = header {
                response.headers_mut().insert("PAYMENT-RESPONSE", header);
            }
        }

        response
    }
}
