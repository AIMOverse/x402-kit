//! # X402 Kit v1 Protocol Implementation
//!
//! This module contains the v1 implementation of the X402 protocol, including transport types,
//! seller utilities, facilitator client, and signer logic.
//!
//! ## Core Components
//!
//! - **[`transport`]**: Types and traits for defining X402 transport mechanisms and facilitator interactions.
//!
//! ### Commonly-Used Utilities
//!
//! - **[`seller`]**: Utilities for building X402 sellers, including an Axum integration.
//! - **[`facilitator_client`]**: Utilities for building X402 facilitator clients.
//!
//! ## Seller Guide
//!
//! ### Configuring Payment Requirements
//!
//! #### Resources
//!
//! A [`crate::core::Resource`] defines the details of what is being sold, including its URL, description, MIME type, and optional output schema.
//!
//! ```
//! use url_macro::url;
//! use x402_kit::core::Resource;
//! use x402_kit::types::OutputSchema;
//!
//! let resource = Resource::builder()
//!     .url(url!("http://example.com/premium-content"))
//!     .description("Premium content access")
//!     .mime_type("application/json")
//!     // Make the endpoint discoverable with facilitators
//!     .output_schema(OutputSchema::http_post_discoverable())
//!     .build();
//! ```
//!
//! See unit tests under [`crate::types::OutputSchema`] in the [GitHub repo](https://github.com/AIMOverse/x402-kit/blob/main/crates/x402-kit/src/types/schema.rs) for complex examples of defining input/output schemas.
//!
//! #### Schemes
//!
//! Choose a scheme for building payment requirements. For example, using the `ExactEvm` scheme:
//!
//! ```
//! use alloy_primitives::address;
//! use url_macro::url;
//! use x402_kit::{
//!     core::Resource,
//!     networks::evm::assets::UsdcBase,
//!     schemes::exact_evm::ExactEvm,
//!     v1::transport::PaymentRequirements,
//! };
//!
//! # fn build_payment_requirements() {
//!
//! let resource = Resource::builder()
//!     .url(url!("https://example.com/premium"))
//!     .description("Premium content access")
//!     .mime_type("application/json")
//!     .build();
//!
//! let payment_requirements = ExactEvm::builder()
//!     .asset(UsdcBase)
//!     .amount(1000) // Amount in smallest units (e.g., 1000 = 0.001 USDC)
//!     .pay_to(address!("0x3CB9B3bBfde8501f411bB69Ad3DC07908ED0dE20"))
//!     .resource(resource)
//!     .build();
//!
//! // Convert to PaymentRequirements for use with facilitator using .v1()
//! let requirements: PaymentRequirements = payment_requirements.v1();
//! # }
//! ```
//!
//! ### Axum Integration
//!
//! As a seller, you might be interested in Axum integration for building an X402-enabled server.
//! See [`seller::axum::PaymentHandler`] for more details.
//!
//! ```
//! use alloy_primitives::address;
//! use axum::{
//!     Router,
//!     extract::Request,
//!     middleware::{Next, from_fn},
//!     response::{IntoResponse, Response},
//!     routing::post,
//! };
//! use url_macro::url;
//! use x402_kit::{
//!     core::Resource,
//!     v1::facilitator_client::RemoteFacilitatorClient,
//!     networks::evm::assets::UsdcBase,
//!     schemes::exact_evm::ExactEvm,
//!     v1::seller::axum::PaymentHandler,
//! };
//!
//! async fn payment_middleware(req: Request, next: Next) -> Response {
//!     PaymentHandler::builder(RemoteFacilitatorClient::from_url(
//!         url!("https://facilitator.example.com"),
//!     ))
//!     .add_payment(
//!         ExactEvm::builder()
//!             .asset(UsdcBase)
//!             .amount(1000)
//!             .pay_to(address!("0x17d2e11d0405fa8d0ad2dca6409c499c0132c017"))
//!             .resource(
//!                 Resource::builder()
//!                     .url(url!("http://localhost:3000/premium"))
//!                     .description("Premium content")
//!                     .mime_type("application/json")
//!                     .build(),
//!             )
//!             .build()
//!             .v1(),
//!     )
//!     .build()
//!     .handle_payment()
//!     .req(req)
//!     .next(next)
//!     .call()
//!     .await
//!     .map(|r| r.into_response())
//!     .unwrap_or_else(|err| err.into_response())
//! }
//!
//! # async fn create_app() -> Router {
//! Router::new()
//!     .route("/premium", post(premium_handler).layer(from_fn(payment_middleware)))
//! # }
//! # async fn premium_handler() {}
//! ```
//!
//! ### The Seller Toolkit
//!
//! The seller toolkit provides utilities for building custom payment handling logic outside of specific frameworks.
//!
//! You might be interested in this if you are using a different web framework or have custom requirements.
//!
//! See [`seller::toolkit`] for more details.

pub mod facilitator;
pub mod signer;
pub mod transport;

#[cfg(feature = "seller")]
pub mod seller;

#[cfg(feature = "facilitator-client")]
pub mod facilitator_client;
