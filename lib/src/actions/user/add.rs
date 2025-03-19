use std::ops::Deref;

#[cfg(unix)]
use tracing::debug;

use super::{User, UserVariant};
use crate::{actions::Action, contexts::Contexts, manifests::Manifest, steps::Step};

pub type UserAdd = User;

impl Action for UserAdd {
    fn plan(&self, _manifest: &Manifest, context: &Contexts) -> anyhow::Result<Vec<Step>> {
        let variant: UserVariant = self.into();
        let box_provider = variant.provider.clone().get_provider();
        let provider = box_provider.deref();

        let mut atoms: Vec<Step> = vec![];

        if variant.username.is_empty() {
            return Ok(atoms);
        }

        #[cfg(unix)]
        match uzers::get_user_by_name(&variant.username) {
            Some(_user) => debug!(message = "User already exists", username = ?variant.username),
            None => atoms.append(&mut provider.add_user(&variant, &context)?),
        }

        #[cfg(not(unix))]
        atoms.append(&mut provider.add_user(&variant, &context)?);

        Ok(atoms)
    }
}
