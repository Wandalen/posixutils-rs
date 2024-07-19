use std::collections::HashSet;

use super::Prefix;

/// A recipe configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    /// Whether the errors should be ignored.
    pub ignore: bool,
    /// Whether the recipe should be silent.
    pub silent: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Config {
            ignore: false,
            silent: false,
        }
    }
}

impl From<HashSet<Prefix>> for Config {
    fn from(prefixes: HashSet<Prefix>) -> Self {
        let mut ignore = false;
        let mut silent = false;

        for prefix in prefixes {
            match prefix {
                Prefix::Ignore => ignore = true,
                Prefix::Silent => silent = true,
                Prefix::Execute => todo!(),
            }
        }

        Self { ignore, silent }
    }
}
