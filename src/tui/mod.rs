//! TUI module for VQS Master Grid Designer
//!
//! Interactive terminal application for building LSC2 master grids
//! using the systematic framework.

mod app;
mod colors;
mod event;
mod export;
mod path;
mod preview;
mod stretching;
pub mod suggestions;
mod table;
pub mod ui;

pub use app::{App, ExportOptions, Focus, MeshInfo, OutputFormat, StatusLevel, StatusMessage, StretchingType};
pub use event::{Event, EventHandler};
pub use path::{PathAnchor, PathError, PathSelection};
pub use stretching::{StretchingKind, StretchingParams, ZoneStats, compute_mesh_zone_stats};
pub use suggestions::{Anchor, SuggestionAlgorithm, SuggestionMode, SuggestionParams};
pub use table::{CellValidity, CellValue, ConstructionTable, EditMode};
