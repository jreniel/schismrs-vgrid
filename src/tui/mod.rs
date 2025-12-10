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
mod table;
pub mod ui;

pub use app::{App, ExportOptions, Focus, OutputFormat, StatusLevel, StatusMessage, StretchingType};
pub use event::{Event, EventHandler};
pub use path::{PathAnchor, PathError, PathSelection};
pub use stretching::{StretchingParams, ZoneStats};
pub use table::{CellValidity, CellValue, ConstructionTable, EditMode};
