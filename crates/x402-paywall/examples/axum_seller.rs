use alloy::primitives::address;
use axum::{
    Json, Router,
    extract::{Request, State},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Value, json};
use tower_http::trace::TraceLayer;
use url::Url;
use url_macro::url;
use x402_kit::{
    core::Resource,
    facilitator_client::{FacilitatorClient, StandardFacilitatorClient},
    networks::evm::assets::UsdcBase,
    schemes::exact_evm::ExactEvm,
};
use x402_paywall::paywall::PayWall;

#[derive(Clone)]
struct PayWallState {
    facilitator: StandardFacilitatorClient,
}

async fn paywall(State(state): State<PayWallState>, req: Request, next: Next) -> Response {
    PayWall::builder()
        .facilitator(state.facilitator)
        .accepts(
            ExactEvm::builder()
                .amount(1000)
                .asset(UsdcBase)
                .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
                .build(),
        )
        .resource(
            Resource::builder()
                .url(url!("https://example.com/resource"))
                .description("X402 payment protected resource")
                .mime_type("application/json")
                .build(),
        )
        .build()
        .handle_payment(req, |req| next.run(req))
        .await
        .unwrap_or_else(|err| err.into_response())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let facilitator_url = std::env::var("FACILITATOR_URL")
        .expect("Please set `FACILITATOR_URL` in environment variables");
    let facilitator_url =
        Url::parse(&facilitator_url).expect("FACILITATOR_URL must be a valid URL");
    tracing::info!("Using facilitator at {}", facilitator_url);
    let facilitator = FacilitatorClient::from_url(facilitator_url);
    let state = PayWallState { facilitator };

    let app = Router::new()
        .route(
            "/resource",
            post(example_handler).layer(from_fn_with_state(state, paywall)),
        )
        .layer(TraceLayer::new_for_http());

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16 integer");
    let addr: std::net::SocketAddr = ([0, 0, 0, 0], port).into();

    tracing::info!("Starting server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("Server running at http://{}", addr);
    axum::serve(listener, app).await.expect("Server failed");
}

async fn example_handler() -> Json<Value> {
    Json(json!({
        "message": "You have accessed a protected resource!"
    }))
}
