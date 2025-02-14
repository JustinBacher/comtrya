use crate::{
    actions::Action, atoms::plugin::PluginExec, contexts::Contexts, manifests::Manifest,
    steps::Step,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(JsonSchema, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    name: String,
    #[serde(flatten)]
    config: Value,
}

impl Action for Plugin {
    // FIXME: this should be in the plugin spec
    fn summarize(&self) -> String {
        "I am a plugin".to_string()
    }

    fn plan(&self, _manifest: &Manifest, _context: &Contexts) -> anyhow::Result<Vec<Step>> {
        Ok(vec![Step {
            atom: Box::new(PluginExec(self.config.clone())),
            initializers: vec![],
            finalizers: vec![],
        }])
    }
}
