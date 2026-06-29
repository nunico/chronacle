//! Entity CRUD: create / read / update / delete graph nodes, plus the
//! timeline ordering and embedding helpers that operate on a single node.

mod read;
mod update;
mod write;

pub use read::*;
pub use update::*;
pub use write::*;

#[cfg(test)]
#[path = "crud_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "crud_tests_create.rs"]
mod tests_create;

#[cfg(test)]
#[path = "crud_tests_read.rs"]
mod tests_read;

#[cfg(test)]
#[path = "crud_tests_update.rs"]
mod tests_update;
