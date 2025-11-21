use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    routing::post,
};
use serde::Deserialize;
use url::Url;
use x402_kit::{
    config::Resource,
    facilitator_client::{IntoSettleResponse, RemoteFacilitatorClient},
    networks::evm::assets::UsdcBase,
    schemes::exact_evm::ExactEvmConfig,
    transports::{
        FacilitatorSettleFailed, FacilitatorSettleResponse, FacilitatorSettleSuccess,
        http_seller::process_payment,
    },
};

#[tokio::main]
async fn main() {
    let facilitator_url = std::env::var("FACILITATOR_URL").expect("FACILITATOR_URL not set");
    println!("Using facilitator at {}", facilitator_url);

    // build our application with a route
    let app = Router::new()
        // `POST /premium` goes to `premium_content`
        .route("/premium", post(premium_content))
        .route("/fto", post(facilitator_types_override))
        .with_state(facilitator_url);

    // run our app with hyper, listening globally on port 3000
    println!("Listening on http://0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn premium_content(
    State(facilitator_url): State<String>,
    req: Request,
) -> (StatusCode, Json<serde_json::Value>) {
    println!("Received request for premium content");

    // Define resource
    let resource = Resource::builder()
        .url(Url::parse("http://0.0.0.0:3000/premium").unwrap())
        .description("Premium content")
        .mime_type("application/json")
        .build();

    // Define payment requirements for each transport
    let payment_requirements = ExactEvmConfig::builder()
        .asset(UsdcBase)
        .amount(500) // amount in smallest units
        .pay_to(alloy_primitives::address!(
            "0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"
        ))
        .resource(resource)
        .build()
        .into_payment_requirements();

    // Create facilitator client
    let facilitator = RemoteFacilitatorClient::new_default(Url::parse(&facilitator_url).unwrap());

    // Extract payment header
    let raw_x_payment_header = req.headers().get("X-Payment").map(|v| v.to_str().unwrap());
    println!("Processing payment with header: {:?}", raw_x_payment_header);

    let result = process_payment(
        &facilitator,
        raw_x_payment_header,
        vec![payment_requirements],
    )
    .await;
    println!("Payment processing result {:?}", result);

    match result {
        Ok(response) => {
            println!("Payment successful: {:?}", response);
        }
        Err(err) => {
            return (
                StatusCode::from_u16(err.status).unwrap(),
                Json(serde_json::to_value(err.into_payment_requirements_response()).unwrap()),
            );
        }
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({"message": "Premium content accessed"})),
    )
}

async fn facilitator_types_override(
    State(facilitator_url): State<String>,
    req: Request,
) -> (StatusCode, Json<serde_json::Value>) {
    println!("Received request for premium content");

    let resource = Resource::builder()
        .url(Url::parse("http://0.0.0.0:3000/fto").unwrap())
        .description("Premium content")
        .mime_type("application/json")
        .build();

    let payment_requirements = ExactEvmConfig::builder()
        .asset(UsdcBase)
        .amount(500) // amount in smallest units
        .pay_to(alloy_primitives::address!(
            "0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"
        ))
        .resource(resource)
        .build()
        .into_payment_requirements();

    // Define custom response types

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CustomSettleResponse {
        pub success: bool,
        pub error: Option<String>,
        pub tx_hash: Option<String>,
        pub network_id: Option<String>,
        pub payer: Option<String>,
    }

    // Implement conversion trait

    impl IntoSettleResponse for CustomSettleResponse {
        fn into_settle_response(self) -> FacilitatorSettleResponse {
            if self.success {
                FacilitatorSettleResponse::Success(FacilitatorSettleSuccess {
                    transaction: self.tx_hash.unwrap_or("[Unknown]".to_string()),
                    network: self.network_id.unwrap_or("[Unknown]".to_string()),
                    payer: self.payer.unwrap_or("Unknown".to_string()),
                })
            } else {
                FacilitatorSettleResponse::Failed(FacilitatorSettleFailed {
                    error_reason: self.error.unwrap_or_default(),
                    payer: self.payer,
                })
            }
        }
    }

    // Override the facilitator client with custom types

    let facilitator = RemoteFacilitatorClient::new_default(Url::parse(&facilitator_url).unwrap())
        .with_settle_response_type::<CustomSettleResponse>();

    let raw_x_payment_header = req.headers().get("X-Payment").map(|v| v.to_str().unwrap());
    println!("Processing payment with header: {:?}", raw_x_payment_header);

    let result = process_payment(
        &facilitator,
        raw_x_payment_header,
        vec![payment_requirements],
    )
    .await;
    println!("Payment processing result {:?}", result);

    match result {
        Ok(response) => {
            println!("Payment successful: {:?}", response);
        }
        Err(err) => {
            return (
                StatusCode::from_u16(err.status).unwrap(),
                Json(serde_json::to_value(err.into_payment_requirements_response()).unwrap()),
            );
        }
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({"message": "Premium content accessed"})),
    )
}
