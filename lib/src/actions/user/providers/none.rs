use serde::{Deserialize, Serialize};
use tracing::warn;

use super::UserProvider;
use crate::{
    actions::user::{UserVariant, add_group::UserAddGroup},
    contexts::Contexts,
    steps::Step,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoneUserProvider {}

impl UserProvider for NoneUserProvider {
    fn add_user(&self, _user: &UserVariant, _contexts: &Contexts) -> anyhow::Result<Vec<Step>> {
        warn!("This system does not have a provider for users");
        Ok(vec![])
    }

    fn add_to_group(
        &self, _user: &UserAddGroup, _contexts: &Contexts,
    ) -> anyhow::Result<Vec<Step>> {
        warn!(message = "This system does not have a provider for users");
        Ok(vec![])
    }
}
