use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use url::Url;
use url_macro::url;
use x402_kit::{
    config::Resource, facilitator_client::RemoteFacilitatorClient, networks::evm::assets::UsdcBase,
    schemes::exact_evm::ExactEvm, seller::process_payment, transport::Base64EncodedHeader,
    types::OutputSchema,
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
    let resource = Resource::builder()
        .url(url!("http://0.0.0.0:3000/premium"))
        .description("Premium content")
        .mime_type("application/json")
        .output_schema(OutputSchema::discoverable_http_post())
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
