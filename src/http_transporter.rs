//! HTTP Transporter module for moleculer-rs
//!
//! This module provides HTTP-based transport for moleculer-rs.
//! It re-exports the HTTP transport implementation from the transporter module.
//!
//! # Example
//!
//! ```rust
//! use moleculer::config::{ConfigBuilder, Transporter};
//! use moleculer::http_transporter::HttpTransport;
//!
//! // Create config with HTTP transporter
//! let config = ConfigBuilder::default()
//!     .transporter(Transporter::http("0.0.0.0:8080"))
//!     .build();
//! ```

// Re-export HTTP transport types from the transporter module
pub use crate::transporter::http_transport::{
    parse_http_address, HttpClientTransport, HttpConfig, HttpMessage, HttpResponse, HttpTransport,
};
