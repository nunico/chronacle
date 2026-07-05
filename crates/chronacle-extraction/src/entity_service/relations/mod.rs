//! Relationship edges and graph queries: creating `relates_to` edges (with the
//! specificity-tier collapsing rule), the ego graph, and the flat relations list.

pub(super) mod edge;
mod flat;
mod graph;
mod scope;

pub use edge::{relate, relate_collapsing};
pub use flat::get_entity_relations;
pub use graph::get_entity_graph;
pub(crate) use scope::check_scope;

#[cfg(test)]
#[path = "relations_tests.rs"]
mod relations_tests;

#[cfg(test)]
#[path = "scope_tests.rs"]
mod scope_tests;
