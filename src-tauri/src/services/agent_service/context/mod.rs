//! Retrieval context assembly — resolving subscribed collections, gathering
//! campaign/collection entity notes, and formatting retrieved chunks into the
//! prompt's reference block.

mod entity;
mod format;
mod resolve;
mod rows;

pub use entity::fetch_entity_context;
pub use resolve::resolve_collection_ids;
pub(super) use format::build_context;

#[cfg(test)]
#[path = "context_tests.rs"]
mod context_tests;
#[cfg(test)]
#[path = "context_tests_entity.rs"]
mod context_tests_entity;
