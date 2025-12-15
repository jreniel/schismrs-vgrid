//! TUI module for VQS Master Grid Designer
//!
//! Interactive terminal application for building VQS master grids.

mod app;
mod event;
mod export;
mod path;
mod stretching;
pub mod suggestions;
mod table;
pub mod ui;

pub use app::{AnchorTruncation, App, ExportOptions, Focus, MeshInfo, OutputFormat, ProfileViewMode, StatusLevel, StatusMessage, StretchingType};
pub use event::{Event, EventHandler};
pub use path::{PathAnchor, PathError, PathSelection};
pub use stretching::{StretchingKind, StretchingParams, ZoneStats, compute_mesh_zone_stats};
pub use suggestions::{Anchor, SuggestionAlgorithm, SuggestionMode, SuggestionParams};
pub use table::{ConstructionTable, EditMode};
