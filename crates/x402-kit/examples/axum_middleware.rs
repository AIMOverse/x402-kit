use alloy_primitives::address;
use axum::{
    Extension, Json, Router,
    extract::{Request, State},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::post,
};
use http::StatusCode;
use solana_pubkey::pubkey;
use url::Url;
use url_macro::url;
use x402_kit::{
    config::Resource,
    facilitator_client::RemoteFacilitatorClient,
    networks::{evm::assets::UsdcBase, svm::assets::UsdcSolanaDevnet},
    schemes::{exact_evm::ExactEvm, exact_svm::ExactSvm},
    seller::axum::{PaymentHandler, PaymentProcessingState},
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let facilitator_url = std::env::var("FACILITATOR_URL")
        .expect("FACILITATOR_URL not set")
        .parse()
        .expect("Invalid FACILITATOR_URL");

    tracing::info!("Using facilitator at {}", facilitator_url);

    let state = AppState { facilitator_url };

    // build our application with a route
    let app = Router::new()
        .route(
            "/premium",
            post(premium_content).layer(from_fn_with_state(state.clone(), payment_middleware)),
        )
        .route(
            "/premium/solana",
            post(premium_content)
                .layer(from_fn_with_state(state.clone(), payment_middleware_solana)),
        );

    // run our app with hyper, listening globally on port 3000
    tracing::info!("Listening on http://0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    facilitator_url: Url,
}

async fn payment_middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    PaymentHandler::builder(RemoteFacilitatorClient::from_url(state.facilitator_url))
        .add_payment(
            ExactEvm::builder()
                .asset(UsdcBase)
                .amount(1000)
                .pay_to(address!("0x17d2e11d0405fa8d0ad2dca6409c499c0132c017"))
                .resource(
                    Resource::builder()
                        .url(
                            url!("http://localhost:3000")
                                .join(req.uri().path())
                                .unwrap(),
                        )
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

async fn payment_middleware_solana(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    PaymentHandler::builder(RemoteFacilitatorClient::from_url(state.facilitator_url))
        .add_payment(
            ExactSvm::builder()
                .asset(UsdcSolanaDevnet)
                .amount(1000)
                .pay_to(pubkey!("Ge3jkza5KRfXvaq3GELNLh6V1pjjdEKNpEdGXJgjjKUR"))
                .resource(
                    Resource::builder()
                        .url(
                            url!("http://localhost:3000")
                                .join(req.uri().path())
                                .unwrap(),
                        )
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
