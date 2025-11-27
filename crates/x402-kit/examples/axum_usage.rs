use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use url::Url;
use url_macro::url;
use x402_kit::{
    config::Resource,
    facilitator_client::{IntoSettleResponse, RemoteFacilitatorClient},
    networks::evm::assets::{UsdcBase, UsdcBaseSepolia},
    schemes::exact_evm::ExactEvm,
    seller::process_payment,
    transport::{
        Base64EncodedHeader, FacilitatorPaymentRequest, FacilitatorSettleFailed,
        FacilitatorSettleResponse, FacilitatorSettleSuccess, PaymentPayload, PaymentRequirements,
    },
    types::{Input, InputMethod, InputType, OutputSchema},
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let facilitator_url = std::env::var("FACILITATOR_URL")
        .expect("FACILITATOR_URL not set")
        .parse()
        .expect("Invalid FACILITATOR_URL");

    tracing::info!("Using facilitator at {}", facilitator_url);

    // build our application with a route
    let app = Router::new()
        // `POST /premium` goes to `premium_content`
        .route("/premium", post(premium_content))
        .route("/fto", post(facilitator_types_override))
        .with_state(facilitator_url);

    // run our app with hyper, listening globally on port 3000
    tracing::info!("Listening on http://0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn premium_content(
    State(facilitator_url): State<Url>,
    req: Request,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Received request for premium content");

    // Define resource
    let output_schema = OutputSchema::builder()
        .input(
            Input::builder()
                .discoverable(true)
                .input_type(InputType::Http)
                .method(InputMethod::Post)
                .build(),
        )
        .build();
    let resource = Resource::builder()
        .url(url!("http://0.0.0.0:3000/premium"))
        .description("Premium content")
        .mime_type("application/json")
        .output_schema(output_schema)
        .build();

    // Define payment requirements for each transport
    // You can customize anything here as needed per request
    let payment_requirements = ExactEvm::builder()
        .asset(UsdcBase)
        .amount(500) // amount in smallest units
        .pay_to(alloy_primitives::address!(
            "0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"
        ))
        .resource(resource)
        .build()
        .into();

    // Create facilitator client
    let facilitator = RemoteFacilitatorClient::new_default(facilitator_url);

    // We won't generate ANY errors until here - Everything checked at compile time

    // Process payment with the utility function
    let result = process_payment(&facilitator, req.headers(), vec![payment_requirements])
        .await
        .map_err(|err| {
            (
                err.status,
                Json(serde_json::to_value(err.into_payment_requirements_response()).unwrap()),
            )
        })?;

    tracing::debug!("Payment processing result {:?}", result);

    let recipient = Base64EncodedHeader::try_from(result)
        .ok()
        .and_then(|v| v.to_string().parse().ok());

    let mut response = (
        StatusCode::CREATED,
        Json(serde_json::json!({"message": "Premium content accessed"})),
    )
        .into_response();

    if let Some(header_value) = recipient {
        response
            .headers_mut()
            .insert("X-Payment-Response", header_value);
    }

    Ok(response)
}

async fn facilitator_types_override(
    State(facilitator_url): State<Url>,
    req: Request,
) -> (StatusCode, Json<serde_json::Value>) {
    println!("Received request for premium content");

    let resource = Resource::builder()
        .url(Url::parse("http://0.0.0.0:3000/fto").unwrap())
        .description("Premium content")
        .mime_type("application/json")
        .build();

    let payment_requirements = ExactEvm::builder()
        .asset(UsdcBaseSepolia)
        .amount(500) // amount in smallest units
        .pay_to(alloy_primitives::address!(
            "0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"
        ))
        .resource(resource)
        .build()
        .into();

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

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CustomSettleRequest {
        pub x402_version: i8,
        pub payment_payload: PaymentPayload,
        pub payment_requirements: PaymentRequirements,
    }

    // Implement conversion trait

    impl IntoSettleResponse for CustomSettleResponse {
        fn into_settle_response(self) -> FacilitatorSettleResponse {
            if self.success {
                FacilitatorSettleResponse::Success(FacilitatorSettleSuccess {
                    transaction: self.tx_hash.unwrap_or("[Unknown]".to_string()),
                    network: self.network_id.unwrap_or("[Unknown]".to_string()),
                    payer: self.payer.unwrap_or("[Unknown]".to_string()),
                })
            } else {
                FacilitatorSettleResponse::Failed(FacilitatorSettleFailed {
                    error_reason: self.error.unwrap_or_default(),
                    payer: self.payer,
                })
            }
        }
    }

    impl From<FacilitatorPaymentRequest> for CustomSettleRequest {
        fn from(value: FacilitatorPaymentRequest) -> Self {
            CustomSettleRequest {
                x402_version: 1,
                payment_payload: value.payload.payment_payload,
                payment_requirements: value.payload.payment_requirements,
            }
        }
    }

    // Override the facilitator client with custom types

    let facilitator = RemoteFacilitatorClient::new_default(facilitator_url)
        .with_settle_request_type::<CustomSettleRequest>()
        .with_settle_response_type::<CustomSettleResponse>();

    let raw_x_payment_header = req.headers().get("X-Payment").map(|v| v.to_str().unwrap());
    println!("Processing payment with header: {:?}", raw_x_payment_header);

    let result = process_payment(&facilitator, req.headers(), vec![payment_requirements]).await;
    println!("Payment processing result {:?}", result);

    match result {
        Ok(response) => {
            println!("Payment successful: {:?}", response);
        }
        Err(err) => {
            return (
                err.status,
                Json(serde_json::to_value(err.into_payment_requirements_response()).unwrap()),
            );
        }
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({"message": "Premium content accessed"})),
    )
}
