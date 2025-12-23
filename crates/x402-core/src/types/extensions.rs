use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::types::AnyJson;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    pub info: AnyJson,
    pub schema: AnyJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionIdentifier(pub String);

impl Display for ExtensionIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
