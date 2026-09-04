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
//! - **Responsive policy** (`responsive`): Pure viewport, density, and layout
//!   contracts shared by the other UI layers

pub mod animation;
pub mod components;
pub mod effects;
pub mod icons;
pub mod overlay;
pub mod pages;
pub mod primitives;
pub mod responsive;
pub mod theme;
pub mod widgets;
