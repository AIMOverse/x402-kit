//! # X402 Paywall
//!
//! A framework-agnostic HTTP paywall implementation for the X402 payment protocol.
//!
//! This crate provides [`PayWall`](paywall::PayWall), a composable middleware that protects
//! HTTP resources with X402 payments. It handles the complete payment lifecycle including
//! verification and settlement through a configured facilitator.
//!
//! ## Quick Start
//!
//! ```rust
//! use alloy::primitives::address;
//! use url_macro::url;
//! use x402_kit::{
//!     core::Resource,
//!     facilitator_client::FacilitatorClient,
//!     networks::evm::assets::UsdcBaseSepolia,
//!     schemes::exact_evm::ExactEvm,
//! };
//! use x402_paywall::paywall::PayWall;
//!
//! let facilitator = FacilitatorClient::from_url(url!("https://facilitator.example.com"));
//!
//! let paywall = PayWall::builder()
//!     .facilitator(facilitator)
//!     .accepts(
//!         ExactEvm::builder()
//!             .amount(1000)
//!             .asset(UsdcBaseSepolia)
//!             .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
//!             .build(),
//!     )
//!     .resource(
//!         Resource::builder()
//!             .url(url!("https://example.com/resource"))
//!             .description("Protected resource")
//!             .mime_type("application/json")
//!             .build(),
//!     )
//!     .build();
//! ```
//!
//! ## Modules
//!
//! - [`paywall`]: The main [`PayWall`](paywall::PayWall) struct and payment flow logic.
//! - [`processor`]: Payment processing types including [`RequestProcessor`](processor::RequestProcessor)
//!   and [`PaymentState`](processor::PaymentState).
//! - [`errors`]: Error types for payment failures and HTTP error responses.
//!
//! ## Payment Flow
//!
//! The standard payment flow using [`PayWall::handle_payment`](paywall::PayWall::handle_payment):
//!
//! 1. **Update Accepts**: Filter payment requirements based on facilitator support.
//! 2. **Process Request**: Extract and validate the `PAYMENT-SIGNATURE` header.
//! 3. **Verify**: Verify the payment signature with the facilitator.
//! 4. **Run Handler**: Execute the resource handler.
//! 5. **Settle**: Settle the payment on successful response.
//!
//! For custom flows, use the step-by-step API directly. See [`PayWall`](paywall::PayWall) for details.
//!
//! ## Framework Integration
//!
//! While framework-agnostic, `x402-paywall` works seamlessly with any HTTP framework.
//! Here's an example with Axum:
//!
//! ```rust,ignore
//! use axum::{
//!     extract::{Request, State},
//!     middleware::{from_fn_with_state, Next},
//!     response::{IntoResponse, Response},
//!     routing::post,
//!     Router,
//! };
//!
//! async fn paywall_middleware(
//!     State(state): State<AppState>,
//!     req: Request,
//!     next: Next,
//! ) -> Response {
//!     let paywall = PayWall::builder()
//!         .facilitator(state.facilitator)
//!         .accepts(/* payment requirements */)
//!         .resource(/* resource config */)
//!         .build();
//!
//!     paywall
//!         .handle_payment(req, |req| next.run(req))
//!         .await
//!         .unwrap_or_else(|err| err.into_response())
//! }
//!
//! let app = Router::new()
//!     .route("/protected", post(handler).layer(from_fn_with_state(state, paywall_middleware)));
//! ```
//!
//! ## Error Handling
//!
//! [`ErrorResponse`](errors::ErrorResponse) implements `IntoResponse` for Axum and can be
//! easily adapted to other frameworks. It returns appropriate HTTP status codes:
//!
//! - `402 Payment Required`: No payment signature provided.
//! - `400 Bad Request`: Invalid payment payload or unsupported requirements.
//! - `500 Internal Server Error`: Facilitator communication failures.

pub mod errors;
pub mod paywall;
pub mod processor;
