use serde::{Deserialize, Serialize};
use tracing::warn;
use which::which;

use super::PackageProvider;
use crate::{
    actions::package::{PackageVariant, repository::PackageRepository},
    atoms::command::Exec,
    contexts::Contexts,
    steps::Step,
    utilities,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zypper {}

impl PackageProvider for Zypper {
    fn name(&self) -> &str {
        "Zypper"
    }

    fn available(&self) -> bool {
        match which("zypper") {
            Ok(_) => true,
            Err(_) => {
                warn!(message = "zypper not available");
                false
            },
        }
    }

    fn bootstrap(&self, _contexts: &Contexts) -> Vec<Step> {
        vec![]
    }

    fn has_repository(&self, _: &PackageRepository) -> bool {
        false
    }

    fn add_repository(
        &self, _repository: &PackageRepository, _contexts: &Contexts,
    ) -> anyhow::Result<Vec<Step>> {
        Ok(vec![])
    }

    fn query(&self, package: &PackageVariant) -> anyhow::Result<Vec<String>> {
        Ok(package.packages())
    }

    fn install(&self, package: &PackageVariant, contexts: &Contexts) -> anyhow::Result<Vec<Step>> {
        let privilege_provider =
            utilities::get_privilege_provider(&contexts).unwrap_or_else(|| "sudo".to_string());

        Ok(vec![Step {
            atom: Box::new(Exec {
                command: String::from("zypper"),
                arguments: vec![String::from("install"), String::from("-y")]
                    .into_iter()
                    .chain(package.extra_args.clone())
                    .chain(package.packages())
                    .collect(),
                privileged: true,
                privilege_provider: privilege_provider.clone(),
                ..Default::default()
            }),
            initializers: vec![],
            finalizers: vec![],
        }])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{actions::package::providers::PackageProviders, contexts::Contexts};

    #[test]
    fn test_install() {
        let zypper = Zypper {};
        let contexts = Contexts::default();
        let steps = zypper.install(
            &PackageVariant {
                name: Some(String::from("")),
                list: vec![],
                extra_args: vec![],
                provider: PackageProviders::Zypper,
                file: false,
            },
            &contexts,
        );

        assert_eq!(steps.unwrap().len(), 1);
    }
}
