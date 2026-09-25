//! Browser transport: no submitted metadata is ever accepted as server truth.
pub mod api;
pub mod pages;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preview {
    pub domain: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub software: String,
    pub users: Option<i64>,
    pub registration_open: Option<bool>,
}
