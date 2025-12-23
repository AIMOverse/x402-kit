use std::pin::Pin;

use axum::{
    Json,
    extract::Request,
    response::{IntoResponse, Response},
};
use tower::{Layer, Service};
use x402_kit::facilitator::Facilitator;

use crate::paywall::{PayWall, PayWallErrorResponse};

impl<F: Facilitator + Clone, S> Layer<S> for PayWall<F> {
    type Service = PayWallService<F, S>;

    fn layer(&self, inner: S) -> Self::Service {
        PayWallService {
            paywall: self.clone(),
            inner,
        }
    }
}

pub struct PayWallService<F: Facilitator, S> {
    paywall: PayWall<F>,
    inner: S,
}

pub type JsonPayWallError = PayWallErrorResponse;

impl IntoResponse for JsonPayWallError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self.body)).into_response();
        if let Some((name, val)) = self.header.header_value() {
            response.headers_mut().insert(name, val);
        }
        response
    }
}

impl<F, S> Service<Request> for PayWallService<F, S>
where
    F: Facilitator + Send + Sync,
    S: Service<Request, Response = Response> + Send,
    S::Future: Send + 'static,
    S::Error: IntoResponse,
{
    type Response = Response;
    type Error = Response;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|err| err.into_response())
    }

    fn call(&mut self, request: Request) -> Self::Future {
        Box::pin(async {
            let response = self
                .paywall
                .handle_payment(request, |req| async {
                    self.inner
                        .call(req)
                        .await
                        .unwrap_or_else(|err| err.into_response())
                })
                .await
                .unwrap_or_else(|err| err.into_response());

            Ok(response)
        })
    }
}
