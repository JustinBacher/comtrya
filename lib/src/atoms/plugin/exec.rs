use std::{process::Stdio, sync::Arc};

use anyhow::{anyhow, Context, Result};
use tracing::{debug, error, trace};

use super::super::Atom;
use crate::atoms::Outcome;
use mlua::IntoLua;
use serde_json::Value;

#[derive(Default)]
pub struct PluginExec(pub Value);

#[allow(dead_code)]
impl Atom for PluginExec {
    fn plan(&self) -> anyhow::Result<Outcome> {
        // NOTE: Should these be left up to plugin to decide?
        Ok(Outcome {
            side_effects: vec![],
            should_run: true,
        })
    }

    fn execute(&mut self) -> Result<()> {
        Ok(())
    }

    fn output_string(&self) -> String {
        self.0.to_string()
    }
}

impl std::fmt::Display for PluginExec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: This should come from the plugin spec
        write!(f, "Plugin: {}", self.0)
    }
}
