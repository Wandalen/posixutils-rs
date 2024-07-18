use super::Prefix;

/// A recipe configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    /// Whether the errors should be ignored.
    pub ignore: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Config { ignore: false }
    }
}

impl From<Option<Prefix>> for Config {
    fn from(prefix: Option<Prefix>) -> Self {
        let mut ignore = false;

        if let Some(prefix) = prefix {
            match prefix {
                Prefix::Ignore => ignore = true,
                Prefix::Silent => todo!(),
                Prefix::Execute => todo!(),
            }
        }

        Self { ignore }
    }
}
