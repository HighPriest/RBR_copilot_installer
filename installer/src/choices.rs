//! Scene 5's model: one pick each of `language`, `codriver` and `style` from
//! what scene 4 committed; every other checked package installs as-is.
//! Option labels are package paths, which are server data and never translated.

mod choice_list;
mod configuration;

pub use choice_list::{ChoiceList, ChoiceOption};
pub use configuration::Configuration;

#[cfg(test)]
mod tests;
