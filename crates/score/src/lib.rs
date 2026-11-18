//! The `score` crate of Bowlingbox.

use std::fmt;

/// A greeting for one name.
///
/// ```
/// use bowlingbox_score::Greeting;
///
/// assert_eq!(Greeting::new("Ada").to_string(), "Hello, Ada!");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Greeting {
    name: String,
}

impl Greeting {
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl fmt::Display for Greeting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hello, {}!", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_the_name() {
        assert_eq!(Greeting::new("world").to_string(), "Hello, world!");
    }
}
