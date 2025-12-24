//! This module defines types related to X402 protocol extensions.

use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::types::AnyJson;

/// Represents an extension in the X402 protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    /// The information about the extension.
    pub info: AnyJson,
    /// The schema defining the extension's structure.
    pub schema: AnyJson,
}

/// Represents the identifier for an extension in the X402 protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionIdentifier(pub String);

impl Display for ExtensionIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
