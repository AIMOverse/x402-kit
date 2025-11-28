use alloy_primitives::address;
use axum::{
    Extension, Json, Router,
    extract::Request,
    middleware::{Next, from_fn},
    response::{IntoResponse, Response},
    routing::post,
};
use http::StatusCode;
use url_macro::url;
use x402_kit::{
    config::Resource,
    facilitator_client::RemoteFacilitatorClient,
    networks::evm::assets::UsdcBase,
    schemes::exact_evm::ExactEvm,
    seller::axum::{PaymentHandler, PaymentProcessingState},
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new().route(
        "/premium",
        post(premium_content).layer(from_fn(payment_middleware)),
    );

    // run our app with hyper, listening globally on port 3000
    tracing::info!("Listening on http://0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn payment_middleware(req: Request, next: Next) -> Response {
    PaymentHandler::builder(RemoteFacilitatorClient::from_url(
        std::env::var("FACILITATOR_URL")
            .expect("FACILITATOR_URL not set")
            .parse()
            .expect("Invalid FACILITATOR_URL"),
    ))
    .add_payment(
        ExactEvm::builder()
            .asset(UsdcBase)
            .amount(1000)
            .pay_to(address!("0x17d2e11d0405fa8d0ad2dca6409c499c0132c017"))
            .resource(
                Resource::builder()
                    .url(url!("http://localhost:3000/premium"))
                    .description("")
                    .mime_type("")
                    .build(),
            )
            .build(),
    )
    .build()
    .handle_payment()
    .req(req)
    .next(next)
    .call()
    .await
    .map(|r| r.into_response())
    .unwrap_or_else(|err| err.into_response())
}

async fn premium_content(
    Extension(payment): Extension<PaymentProcessingState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match payment {
        PaymentProcessingState::Settled(settled) => tracing::info!(
            "Payment settled: {}",
            serde_json::to_string_pretty(&settled).unwrap()
        ),
        _ => {
            tracing::error!("Payment should be settled here");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    tracing::info!(
        "Received body: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    // Do work to deliver premium content here...

    Ok(Json(serde_json::json!({
        "message": "This is premium content accessible after payment!"
    })))
}
