//! UI module for the music streaming application
//! Dark mode aesthetic with neon pink accents
//!
//! # Architecture
//!
//! The UI is organized into three layers:
//!
//! - **Primitives** (`primitives`): Low-level Widget trait implementations
//! - **Widgets** (`widgets`): Composable UI patterns without business logic
//! - **Components** (`components`): Business-specific UI with Message handling

pub mod animation;
pub mod components;
pub mod effects;
pub mod icons;
pub mod pages;
pub mod primitives;
pub mod theme;
pub mod widgets;

// Re-export interlude_dots from shader/lyrics_engine for backward compatibility
