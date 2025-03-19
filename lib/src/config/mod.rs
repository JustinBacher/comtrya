use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::contexts::privilege::Privilege;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub manifest_paths: Vec<String>,

    #[serde(default)]
    pub variables: BTreeMap<String, String>,

    #[serde(default)]
    pub include_variables: Option<Vec<String>>,

    #[serde(default)]
    pub disable_update_check: bool,

    #[serde(default)]
    pub privilege: Privilege,
}
