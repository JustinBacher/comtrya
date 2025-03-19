use std::collections::HashMap;

use super::Initializer;

#[derive(Clone, Debug)]
pub struct SetEnvVars(pub HashMap<String, String>);

impl Initializer for SetEnvVars {
    fn initialize(&self) -> anyhow::Result<bool> {
        for (key, value) in self.0.iter() {
            unsafe { std::env::set_var(key, value) };
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_env_vars() {
        let map = HashMap::from([("hello".to_string(), "world".to_string())]);
        let initializer = SetEnvVars(map);
        let result = initializer.initialize();

        pretty_assertions::assert_eq!(true, result.is_ok());
        pretty_assertions::assert_eq!(true, result.unwrap());
        let value = env::var("hello");
        pretty_assertions::assert_eq!("world", value.unwrap());
    }
}
