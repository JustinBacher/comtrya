use std::collections::HashMap;

use crate::{atoms::Atom, steps::finalizers::Finalizer};

#[derive(Clone, Debug)]
pub struct RemoveEnvVars(pub HashMap<String, String>);

impl Finalizer for RemoveEnvVars {
    fn finalize(&self, _atom: &dyn Atom) -> anyhow::Result<bool> {
        for (key, _value) in self.0.iter() {
            unsafe { std::env::remove_var(key) };
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;
    use crate::atoms::Echo;

    #[test]
    fn test_env_vars() {
        let atom = Echo("goodbye-world");
        unsafe { env::set_var("FOO", "bar") };

        let map = HashMap::from([("FOO".to_string(), "bar".to_string())]);
        let finalizer = RemoveEnvVars(map);
        let result = finalizer.finalize(&atom);

        pretty_assertions::assert_eq!(true, result.is_ok());
        pretty_assertions::assert_eq!(true, result.unwrap());
        pretty_assertions::assert_eq!(true, std::env::var("FOO").is_err());
    }
}
