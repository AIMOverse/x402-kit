
# X402 Kit

[![Build status](https://github.com/AIMOverse/x402-kit/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/AIMOverse/x402-kit/actions)
[![Crates.io](https://img.shields.io/crates/v/x402-kit)](https://crates.io/crates/x402-kit)
[![Documentation](https://docs.rs/x402-kit/badge.svg)](https://docs.rs/x402-kit)
[![Twitter Follow](https://img.shields.io/twitter/follow/AiMoNetwork?style=social)](https://x.com/AiMoNetwork)
[![Discord](https://img.shields.io/badge/Discord-Join%20AiMoNetwork-5865F2?style=flat&logo=discord&logoColor=white)](https://discord.gg/G3zVrZDa5C)




A fully modular, framework-agnostic, easy-to-extend SDK for building complex X402 payment integrations.

## 📚 Developer Docs

Docs are available at [docs.rs](https://docs.rs/x402-kit/latest/x402_kit/)

## 💡 Core Pain Points Solved

X402-kit is **not a facilitator** — it's a composable SDK for buyers (signers) and sellers (servers) to build custom business logic. Future support for modular facilitator components is planned.

### Beyond Static Pricing and Payment Gateway Middlewares

Existing X402 SDKs only support static prices per API route. X402-kit's fully modular architecture enables complex, dynamic pricing logic while maximizing code reuse.

### Complete Modularity

All internal fields and methods are public by design. Compose and extend functionality freely without fighting the framework.

### Layered Type Safety

- **Transport Layer**: Uses generalized `String` types to prevent serialization failures and ensure service availability
- **Network + Scheme Layer**: Leverages traits and generics for compile-time type checking without runtime overhead

### Ship New Networks Without PRs

Implement a new asset, network, or scheme entirely in your codebase and plug it into the SDK immediately—no upstream pull request or waiting period required thanks to trait-driven extension points.

However, we still recommend contributing back any useful implementations to the main repository to help grow the ecosystem!

### Production-Ready Design

Minimize runtime errors through compile-time guarantees while maintaining the flexibility needed for real-world business logic.

## 🧪 Axum Usage Examples

Two runnable demos live under `crates/x402-kit/examples`. Export `FACILITATOR_URL` so the SDK can reach your facilitator before starting either server.

### 1. Premium content flow (`examples/axum_middleware.rs`, `POST /premium`)

```bash
FACILITATOR_URL=https://your-facilitator.example \
  cargo run -p x402-kit --example axum_middleware
```

`axum_middleware.rs` layers `seller::axum::PaymentHandler` in front of your handler, ensuring requests only reach your business logic once the facilitator settles payment. The middleware also injects `PaymentProcessingState` so downstream handlers can inspect what happened during verification/settlement.

```rust
async fn payment_middleware(req: Request, next: Next) -> Response {
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
            .pay_to(alloy_primitives::address!(
                "0x17d2e11d0405fa8d0ad2dca6409c499c0132c017"
            ))
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
    .map(|success| success.into_response())
    .unwrap_or_else(|err| err.into_response())
}
```

### 2. Seller toolkit helper (`examples/seller_toolkit.rs`, `POST /premium`)

```bash
FACILITATOR_URL=https://your-facilitator.example \
  cargo run -p x402-kit --example seller_toolkit
```

This example shows how to call the lower-level toolkit directly from within a route. It defines a discoverable HTTP resource, builds Exact EVM payment requirements, and hands the inbound headers to `process_payment`. The response that comes back is echoed via `X-Payment-Response`, proving payment to the caller.

```rust
let resource = Resource::builder()
    .url(url!("http://0.0.0.0:3000/premium"))
    .description("Premium content")
    .mime_type("application/json")
    .output_schema(OutputSchema::discoverable_http_post())
    .build();

let payment_requirements = ExactEvm::builder()
    .asset(UsdcBase)
    .amount(500)
    .pay_to(alloy_primitives::address!(
        "0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"
    ))
    .resource(resource)
    .build()
    .into();

let facilitator = RemoteFacilitatorClient::from_url(facilitator_url);

let result = process_payment(&facilitator, req.headers(), vec![payment_requirements])
    .await
    .map_err(|err| {
        (
            err.status,
            Json(serde_json::to_value(err.into_payment_requirements_response()).unwrap()),
        )
    })?;

let mut response = (
    StatusCode::CREATED,
    Json(serde_json::json!({"message": "Premium content accessed"})),
)
    .into_response();

if let Some(header_value) = Base64EncodedHeader::try_from(result)
    .ok()
    .and_then(|encoded| encoded.to_string().parse().ok())
{
    response.headers_mut().insert("X-Payment-Response", header_value);
}
```

### Custom facilitator headers / payloads

If your facilitator expects bespoke request/response bodies, override them via the `with_*_type` helpers while still reusing the same payment definitions and tooling:

```rust
#[derive(Serialize, Deserialize)]
struct CustomSettleRequest { /* ... */ }

impl IntoVerifyResponse for DefaultVerifyResponse {
    fn into_verify_response(self) -> FacilitatorVerifyResponse { /* ... */ }
}

#[derive(Deserialize)]
struct CustomSettleResponse { /* ... */ }

impl IntoSettleResponse for CustomSettleResponse {
    fn into_settle_response(self) -> FacilitatorSettleResponse { /* ... */ }
}

let facilitator = RemoteFacilitatorClient::from_url(facilitator_url)
    .with_settle_request_type::<CustomSettleRequest>()
    .with_settle_response_type::<CustomSettleResponse>();
```

For custom HTTP headers (e.g., API keys, authentication tokens), use `with_header`:

```rust
let facilitator = RemoteFacilitatorClient::from_url(facilitator_url)
    .with_header("X-API-Key", "your-api-key")
    .with_header("Authorization", "Bearer your-token");
```

With custom serialization in place you can continue calling `process_payment` (or the middleware builder) unchanged while swapping transport formats.

## Next Steps

- Full buyer-side signer support (very soon)
- List more networks / assets / schemes into the ecosystem
- MCP / A2A transport support
- X402 V2 support planned

## Contributing

We welcome all contributions to x402-kit! Here's how you can get involved:

- ⭐ **Star** this repository
- 🐛 **Open issues** to report bugs or suggest features
- 🔧 **Submit PRs** to improve the codebase

Contributors will receive **priority access** and **rewards** at AIMO Network's Beta launch (coming soon)!


## Acknowledgements

[x402-rs](https://github.com/x402-rs/x402-rs) for providing the first facilitator and x402 SDK in rust
[faremeter](https://github.com/faremeter/faremeter) for inpiring some of x402-kit's API design
