# X402 Kit

A fully modular SDK for building complex X402 payment integrations.

## Core Pain Points Solved

X402-kit is **not a facilitator** — it's a composable SDK for buyers (signers) and sellers (servers) to build custom business logic. Future support for modular facilitator components is planned.

### 1. Beyond Static Pricing

Existing X402 SDKs only support static prices per API route. X402-kit's fully modular architecture enables complex, dynamic pricing logic while maximizing code reuse.

### 2. Complete Modularity

All internal fields and methods are public by design. Compose and extend functionality freely without fighting the framework.

### 3. Layered Type Safety

- **Transport Layer**: Uses generalized `String` types to prevent serialization failures and ensure service availability
- **Network + Scheme Layer**: Leverages traits and generics for compile-time type checking without runtime overhead

### 4. Production-Ready Design

Minimize runtime errors through compile-time guarantees while maintaining the flexibility needed for real-world business logic.

## Quick Example: Exact EVM Scheme

```rust
use x402_kit::{
    config::Resource,
    networks::evm::assets::UsdcBaseSepolia,
    schemes::exact_evm::ExactEvmConfig,
};
use alloy_primitives::address;
use url::Url;

// Define your payment resource
let resource = Resource::builder()
    .url(Url::parse("https://example.com/payment").unwrap())
    .description("Payment for services".to_string())
    .mime_type("application/json".to_string())
    .build();

// Build payment requirements with type-safe configuration
let config = ExactEvmConfig::builder()
    .asset(UsdcBaseSepolia)
    .amount(1000)
    .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
    .resource(resource)
    .build();

let payment_requirements = config.into_payment_requirements();
```

## Axum Usage Examples

The `crates/x402-kit/examples/axum_usage.rs` example spins up an Axum server that showcases two different ways to accept X402 payments. Run it with your facilitator endpoint configured:

```bash
FACILITATOR_URL=https://your-facilitator.example \
  cargo run -p x402-kit --example axum_usage
```

### 1. Premium content flow (`POST /premium`)

This route uses the default facilitator client and the `process_payment` helper to guard access to premium content. It defines a discoverable HTTP resource and enforces an Exact EVM payment requirement before returning data.

```rust
let output_schema = OutputSchema::builder()
    .input(
        Input::builder()
            .discoverable(true)
            .input_type(InputType::Http)
            .method(InputMethod::Post)
            .build(),
    )
    .build();

let payment_requirements = ExactEvmConfig::builder()
    .asset(UsdcBase)
    .amount(500)
    .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
    .resource(resource)
    .build()
    .into_payment_requirements();

let response = process_payment(
    &facilitator,
    raw_x_payment_header,
    vec![payment_requirements],
)
.await?;
```

The helper returns a typed settle response that can be echoed back via the `X-Payment-Response` header, proving the requester paid before the premium payload is delivered.

### 2. Facilitator types override (`POST /fto`)

This route demonstrates overriding the default facilitator request/response types so you can match bespoke facilitator contracts while still reusing the same payment requirement definition.

```rust
#[derive(Deserialize)]
struct CustomSettleResponse { /* ... */ }

impl IntoSettleResponse for CustomSettleResponse {
    fn into_settle_response(self) -> FacilitatorSettleResponse { /* ... */ }
}

let facilitator = RemoteFacilitatorClient::new_default(facilitator_url)
    .with_settle_request_type::<CustomSettleRequest>()
    .with_settle_response_type::<CustomSettleResponse>();
```

With custom serialization in place you can still invoke `process_payment` exactly the same way, keeping business logic identical while swapping transport formats.

## Mission

Build a fully modular X402 SDK that makes complex payment scenarios simple.
