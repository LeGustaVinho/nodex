//! # Nodex core
//!
//! Language-agnostic project model: loads files through registered [`LanguageFrontend`]s, links
//! per-file IR into a [`StructureGraph`], serves flow graphs, and owns the concept catalog.
//!
//! [`LanguageFrontend`]: nodex_lang_api::LanguageFrontend
//! [`StructureGraph`]: nodex_ir::StructureGraph

pub mod cache;
pub mod canon;
pub mod borrow_origin;
pub mod catalog;
pub mod knowledge;
pub mod link;
pub mod operations;
pub mod perf;
pub mod project;
pub mod semantic;
mod semantic_types;
pub mod navigation;
pub mod units;

pub use catalog::Catalog;
pub use project::{FileSummary, OpenProgress, Project, ProjectSummary};
