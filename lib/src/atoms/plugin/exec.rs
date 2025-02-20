use std::{
    collections::HashMap,
    fmt::{self, Display},
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result};
#[allow(unused_imports)]
use tracing::{debug, error, trace};
use walkdir::WalkDir;

use super::super::Atom;
use crate::atoms::{Outcome, SideEffect};
use crate::utilities::password_manager::PasswordManager;
use dirs_next::config_dir;
use mlua::{prelude::*, FromLuaMulti, Value};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug)]
pub struct PluginExec();

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct PluginSpec {
    is_privileged: Option<bool>,
    spec: HashMap<String, String>,
}

impl FromLuaMulti for PluginSpec {
    fn from_lua_multi(values: mlua::MultiValue, _lua: &mlua::Lua) -> mlua::Result<Self> {
        if let Some(Value::Table(table)) = values.iter().next() {
            Ok(Self {
                is_privileged: table.get("is_privileged")?,
                spec: table.get("spec")?,
            })
        } else {
            Err(mlua::Error::RuntimeError("Expected table".to_string()))
        }
    }
}

impl Display for PluginSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PluginSpec")
    }
}

impl PluginExec {
    fn load_plugins(&self, dir: &PathBuf) -> Result<Vec<PluginSpec>> {
        let plugin_dir = WalkDir::new(dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension() == Some(std::ffi::OsStr::new("lua")));

        let lua = Lua::new();
        let mut specs = vec![];
        for dir_entry in plugin_dir {
            if !dir_entry.file_type().is_file() {
                continue;
            }

            // TODO: need to get this from the runtime
            let Some(contents) = dir_entry.path().to_str() else {
                continue;
            };
            let path = fs::read_to_string(contents)?;

            specs.push(lua.load(path).eval::<PluginSpec>()?);
        }

        Ok(specs)
    }

    pub fn run_function() {
        todo!()
    }
}

#[allow(dead_code)]
#[async_trait::async_trait]
impl Atom for PluginExec {
    fn plan(&self) -> Result<Outcome> {
        let plugin_dir = config_dir()
            .context("Could not get config directory")?
            .to_path_buf()
            .join("comtrya")
            .join("plugins");

        if !plugin_dir.exists() {
            return Ok(Outcome::default());
        }
        let plugins = self.load_plugins(&plugin_dir)?;

        let outcome = Outcome {
            side_effects: vec![SideEffect::Plugins(plugins)],
            should_run: true,
        };
        Ok(outcome)
    }

    async fn execute(&mut self, _: Option<PasswordManager>) -> Result<()> {
        Ok(())
    }

    fn output_string(&self) -> String {
        // TODO: Need to make this more descriptive
        "Plugin".to_string()
    }
}

impl std::fmt::Display for PluginExec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NOTE: This should come from the plugin spec right?
        write!(f, "Plugin: {:?}", self)
    }
}
