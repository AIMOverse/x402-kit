use alloy_primitives::address;
use axum::{
    Json, Router,
    extract::Request,
    middleware::{Next, from_fn},
    routing::post,
};
use url_macro::url;
use x402_kit::{
    config::Resource,
    facilitator_client::RemoteFacilitatorClient,
    networks::evm::assets::UsdcBase,
    schemes::exact_evm::ExactEvm,
    seller::axum::{PaymentErrorResponse, PaymentHandler, PaymentSuccessResponse},
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new()
        // `POST /premium` goes to `premium_content`
        .route(
            "/premium",
            post(premium_content).layer(from_fn(payment_middleware)),
        );

    // run our app with hyper, listening globally on port 3000
    tracing::info!("Listening on http://0.0.0.0:3010");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3010").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn payment_middleware(
    req: Request,
    next: Next,
) -> Result<PaymentSuccessResponse, PaymentErrorResponse> {
    PaymentHandler::builder(RemoteFacilitatorClient::new_default(
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
                    .url(url!("http://localhost:3010/premium"))
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
}

async fn premium_content(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    tracing::info!(
        "Received body: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    // In a real application, you would verify payment here before serving content

    Json(serde_json::json!({
        "message": "This is premium content accessible after payment!"
    }))
}
