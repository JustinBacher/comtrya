use std::{
    fmt::{self, Display, Formatter},
    fs,
    hash::{Hash, Hasher},
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{anyhow, Context, Result};
use dirs_next::data_local_dir;
use gix::{
    bstr::ByteSlice,
    diff::object::tree::EntryKind,
    interrupt::IS_INTERRUPTED,
    open, prepare_clone_bare,
    progress::Discard,
    remote::{ref_map, Direction::Fetch},
    Repository,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_with::{serde_as, DisplayFromStr, KeyValueMap};
use tracing::{debug, info};

use super::{get_plugin, PluginRuntimeSpec};
use crate::{
    actions::Action, atoms::plugin::PluginExec, contexts::Contexts, manifests::Manifest,
    steps::Step, utilities::lua::json_to_lua, utilities::CustomPathBuf,
};

#[derive(
    JsonSchema, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct RepoUri {
    pub username: String,
    pub repo: String,
}

impl Display for RepoUri {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.username, self.repo)
    }
}

impl FromStr for RepoUri {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        debug!("Trying to parse repo uri: {}", s);
        match s.split_once('/') {
            Some((username, repo)) => Ok(Self {
                username: username.to_string(),
                repo: repo.to_string(),
            }),
            _ => Err(anyhow!("repo must be in format 'username/repo'")),
        }
    }
}

pub trait Source {
    fn source(&self) -> Result<String>;
}

#[derive(
    JsonSchema, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(untagged, rename_all = "lowercase")]
pub enum Version {
    #[default]
    Stable,

    #[serde(alias = "*")]
    Latest,

    #[serde(untagged)]
    Tagged(String),
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let version = match self {
            Version::Stable => "stable",
            Version::Latest => "latest",
            Version::Tagged(tag) => tag.as_str(),
        };

        write!(f, "{}", version)
    }
}

#[serde_as]
#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Eq, PartialOrd, Ord)]
pub struct Repo {
    #[serde(alias = "repository")]
    #[serde_as(as = "DisplayFromStr")]
    repo: RepoUri,

    #[serde(alias = "tag")]
    version: Version,
}

impl Display for Repo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.repo, self.version)
    }
}
impl Default for Repo {
    fn default() -> Self {
        Self {
            repo: RepoUri::default(),
            version: Version::Stable,
        }
    }
}

impl PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        self.path().ok() == other.path().ok()
    }
}

impl Hash for Repo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Ok(path) = self.path() {
            path.hash(state);
        } else {
            self.repo.to_string().hash(state);
            self.version.hash(state);
        }
    }
}

impl Repo {
    fn path(&self) -> Result<PathBuf> {
        let path = data_local_dir().context("Failed to locate local data directory")?;

        let plugins_path = path
            .join("comtrya")
            .join("plugins")
            .join(self.repo.to_string());

        if !plugins_path.exists() {
            fs::create_dir_all(&plugins_path)?;
        }

        plugins_path.canonicalize().map_err(anyhow::Error::from)
    }
}

fn checkout(uri: &RepoUri, path: &Path) -> Result<Repository> {
    info!("Checking out plugin");
    let (checkout_result, _) = prepare_clone_bare(format!("https://github.com/{}", &uri), path)?
        .with_remote_name("main")?
        .fetch_then_checkout(Discard, &IS_INTERRUPTED)?;

    Ok(checkout_result.persist())
}

impl Source for Repo {
    fn source(&self) -> Result<String> {
        let path = self.path()?;

        let repo = match open(&path) {
            Ok(r) => r,
            Err(_) => checkout(&self.repo, &path)?,
        };

        if repo.is_dirty()? {
            let remote = repo.find_remote("main")?;
            let fetch = remote
                .connect(Fetch)?
                .prepare_fetch(Discard, ref_map::Options::default())?;
            fetch.receive(Discard, &IS_INTERRUPTED)?;
        }

        let tree = match self.version {
            Version::Stable => match repo.find_reference("tags/latest") {
                Ok(mut reference) => reference.peel_to_tree()?,
                Err(_) => repo.head_tree()?,
            },
            Version::Latest => repo.head_tree()?,
            Version::Tagged(ref version) => repo
                .find_reference(&format!("tags/{}", version))?
                .peel_to_tree()?,
        };

        match tree.find_entry("plugin.lua") {
            Some(entry) if entry.inner.mode.kind() == EntryKind::Blob => {
                Ok(entry.object()?.data.to_str_lossy().to_string())
            }
            _ => Err(anyhow!("No plugin.lua found")),
        }
    }
}

#[derive(JsonSchema, Serialize, Deserialize, Debug, Clone, Default, Eq, PartialOrd, Ord)]
pub struct Dir {
    #[serde(alias = "path")]
    dir: CustomPathBuf,
}

impl Display for Dir {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl Deref for Dir {
    type Target = CustomPathBuf;

    fn deref(&self) -> &Self::Target {
        &self.dir
    }
}

impl PartialEq for Dir {
    fn eq(&self, other: &Self) -> bool {
        self.canonicalize().ok() == other.canonicalize().ok()
    }
}

impl Hash for Dir {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.canonicalize() {
            Ok(path) => path.hash(state),
            _ => self.dir.hash(state),
        };
    }
}

impl Source for Dir {
    fn source(&self) -> Result<String> {
        fs::read_to_string(self.canonicalize()?).context("Failed to read file")
    }
}

#[derive(
    JsonSchema, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(untagged)]
pub enum RepoOrDir {
    Repo(Repo),
    Dir(Dir),
    #[default]
    Invalid,
}

impl Display for RepoOrDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoOrDir::Repo(repo) => write!(f, "{}", repo),
            RepoOrDir::Dir(dir) => write!(f, "{}", dir),
            RepoOrDir::Invalid => write!(f, "Invalid Repo or Directory"),
        }
    }
}

impl Source for RepoOrDir {
    fn source(&self) -> Result<String> {
        match self {
            RepoOrDir::Repo(repo) => repo.source(),
            RepoOrDir::Dir(dir) => dir.source(),
            RepoOrDir::Invalid => Err(anyhow!("Not a valid source")),
        }
    }
}

#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaggedTable {
    #[serde(rename = "$key$")]
    pub action_name: String,

    #[serde(flatten)]
    pub table: JsonValue,
}

impl Deref for TaggedTable {
    type Target = JsonValue;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

#[serde_as]
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Plugin {
    #[serde(flatten, default)]
    pub source: RepoOrDir,

    #[serde(alias = "acts")]
    #[serde_as(as = "KeyValueMap<_>")]
    pub actions: Vec<TaggedTable>,
}

impl Plugin {
    fn runtime(&self, contexts: Option<Contexts>) -> Result<PluginRuntimeSpec> {
        get_plugin(&self.source, contexts).map_err(anyhow::Error::from)
    }
}

impl Display for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Plugin: {:?}", self)
    }
}

impl Action for Plugin {
    fn summarize(&self) -> String {
        self.runtime(None)
            .map(|p| p.spec.summary())
            .unwrap_or("Ran plugin".to_string())
    }

    fn plan(&self, _manifest: &Manifest, context: &Contexts) -> Result<Vec<Step>> {
        let runtime = self.runtime(Some(context.to_owned()))?;
        let planned = self.actions.iter().filter_map(|opt| {
            json_to_lua(&opt.table, &runtime.lua)
                .ok()
                .and_then(|v| {
                    runtime
                        .plan_action(&opt.action_name, v)
                        .and_then(Result::ok)
                })
                .map(|_| opt)
        });

        Ok(planned
            .cloned()
            .map(|opt| Step {
                atom: Box::new(PluginExec {
                    exec_name: opt.action_name,
                    runtime: runtime.clone(),
                    spec: opt.table.clone(),
                }),
                initializers: vec![],
                finalizers: vec![],
            })
            .collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::contexts::build_contexts;
    use crate::manifests::Manifest;
    use serde_json::json;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn plugin_can_plan() -> Result<()> {
        // Create a temporary directory
        let temp_dir = tempdir().expect("Failed to create temp dir");

        // Create a plugin.lua file inside the temporary directory
        let lua_file_path = temp_dir.path().join("plugin.lua");
        let mut lua_file = File::create(&lua_file_path).expect("Failed to create lua file");
        lua_file
            .write_all(
                r#"
return {
    name = "echo",
    summary = "Echoes text",
    actions = {
        echo = {
            plan = function() end,
            exec = function(output, wait)
                print(tostring(output.output))
            end,
        },
    },
}
                "#
                .as_bytes(),
            )
            .expect("Failed to write to lua file");

        let manifest = Manifest::deserialize(json!({
            "actions": [{
                "action": "plugin",
                "dir": lua_file_path,
                "opts": {
                    "echo": { "output": "foo" }
                }
            }]
        }))?;

        let mut steps = manifest.actions.first().unwrap().plan(
            &manifest,
            &build_contexts(&Config {
                ..Default::default()
            }),
        )?;

        assert_eq!(steps.len(), 1);

        steps.first_mut().unwrap().atom.execute()?;

        // Clean up the temporary directory
        temp_dir.close().expect("Failed to close temp dir");

        Ok(())
    }
}
