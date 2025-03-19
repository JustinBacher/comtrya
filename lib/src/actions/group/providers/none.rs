use serde::{Deserialize, Serialize};
use tracing::warn;

use super::GroupProvider;
use crate::{actions::group::GroupVariant, contexts::Contexts, steps::Step};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoneGroupProvider {}

impl GroupProvider for NoneGroupProvider {
    fn add_group(&self, _group: &GroupVariant, _contexts: &Contexts) -> Vec<Step> {
        warn!("This system does not have a provider for groups");
        vec![]
    }
}
