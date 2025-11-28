use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::types::Record;

#[derive(Builder, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDefinition {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub field_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<FieldRequired>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub field_enum: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Record<FieldDefinition>>,
}

impl TryFrom<serde_json::Value> for FieldDefinition {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldRequired {
    Boolean(bool),
    VecString(Vec<String>),
}

#[derive(Builder, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub discoverable: bool,

    #[serde(rename = "type")]
    pub input_type: InputType,

    pub method: Method,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_type: Option<InputBodyType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_params: Option<Record<FieldDefinition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_fields: Option<Record<FieldDefinition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_fields: Option<Record<FieldDefinition>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputType {
    #[serde(rename = "http")]
    Http,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    #[serde(rename = "get")]
    Get,
    #[serde(rename = "post")]
    Post,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputBodyType {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "form-data")]
    FormData,
    #[serde(rename = "multipart-form-data")]
    MultipartFormData,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "binary")]
    Binary,
    #[serde(rename = "event-stream")]
    EventStream,
}

#[derive(Builder, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputSchema {
    pub input: Input,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Record<FieldDefinition>>,
}

impl OutputSchema {
    pub fn discoverable_http_get() -> Self {
        Self::builder()
            .input(
                Input::builder()
                    .input_type(InputType::Http)
                    .method(Method::Get)
                    .discoverable(true)
                    .build(),
            )
            .build()
    }

    pub fn discoverable_http_post() -> Self {
        Self::builder()
            .input(
                Input::builder()
                    .input_type(InputType::Http)
                    .method(Method::Post)
                    .discoverable(true)
                    .build(),
            )
            .build()
    }
}
