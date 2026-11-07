//! # Nodex core
//!
//! Language-agnostic project model: loads files through registered [`LanguageFrontend`]s, links
//! per-file IR into a [`StructureGraph`], serves flow graphs, and owns the concept catalog.
//!
//! [`LanguageFrontend`]: nodex_lang_api::LanguageFrontend
//! [`StructureGraph`]: nodex_ir::StructureGraph

pub mod canon;
pub mod catalog;
pub mod link;
pub mod project;
pub mod semantic;

pub use catalog::Catalog;
pub use project::{FileSummary, Project, ProjectSummary};
