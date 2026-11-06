//! The command registry. Ids follow Magic's domain structure (`domain.verb`); every user-visible
//! action is a command dispatched through [`crate::Session::execute`], the egui UI, the CLI,
//! the control channel and the MCP server all go through it.

pub mod character;
pub mod film;
pub mod project;
pub mod song;
pub mod storyboard;
pub mod words;
