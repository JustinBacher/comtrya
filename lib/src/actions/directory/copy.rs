use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::DirectoryAction;
use crate::{
    actions::Action, atoms::command::Exec, contexts::Contexts, manifests::Manifest, steps::Step,
};

#[derive(JsonSchema, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryCopy {
    pub from: String,
    pub to: String,
}

impl DirectoryCopy {}

impl DirectoryAction for DirectoryCopy {}

#[cfg(target_family = "windows")]
impl Action for DirectoryCopy {
    fn summarize(&self) -> String {
        format!("Copying {} to {}", self.from, self.to)
    }

    fn plan(&self, manifest: &Manifest, _context: &Contexts) -> anyhow::Result<Vec<Step>> {
        let from: String = self.resolve(manifest, &self.from).display().to_string();

        Ok(vec![Step {
            atom: Box::new(Exec {
                command: String::from("Xcopy"),
                arguments: vec!["/E".to_string(), "/I".to_string(), from, self.to.clone()],
                ..Default::default()
            }),
            initializers: vec![],
            finalizers: vec![],
        }])
    }
}

#[cfg(target_family = "unix")]
impl Action for DirectoryCopy {
    fn summarize(&self) -> String {
        format!("Copying {} to {}", self.from, self.to)
    }

    fn plan(&self, manifest: &Manifest, _context: &Contexts) -> anyhow::Result<Vec<Step>> {
        let mut from: String = self.resolve(manifest, &self.from).display().to_string();

        if self.to.ends_with("/") {
            from += "/."
        }

        Ok(vec![
            Step {
                atom: Box::new(Exec {
                    command: String::from("mkdir"),
                    arguments: vec![String::from("-p"), self.to.clone()],
                    ..Default::default()
                }),
                initializers: vec![],
                finalizers: vec![],
            },
            Step {
                atom: Box::new(Exec {
                    command: String::from("cp"),
                    arguments: vec![String::from("-r"), from, self.to.clone()],
                    ..Default::default()
                }),
                initializers: vec![],
                finalizers: vec![],
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{actions::Actions, manifests::Manifest};

    fn get_manifest_dir() -> PathBuf {
        std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples")
            .join("directory")
            .join("copy")
    }

    #[test]
    fn it_can_be_deserialized() {
        let example_yaml = std::fs::File::open(get_manifest_dir().join("dircopy.yaml")).unwrap();
        let mut manifest: Manifest = serde_yml::from_reader(example_yaml).unwrap();

        match manifest.actions.pop() {
            Some(Actions::DirectoryCopy(action)) => {
                assert_eq!("mydir", action.action.from);
                assert_eq!("/tmp/dircopy", action.action.to);
            },
            _ => {
                panic!("DirectoryCopy didn't deserialize to the correct type");
            },
        };
    }
}
