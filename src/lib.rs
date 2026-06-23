//! depot — a developer disk-space reclaimer and shared package store.
//!
//! The crate is usable both as a binary (`depot`) and as a library: the
//! [`scan`] module exposes the detection engine, [`report`] formats results,
//! and [`hub`] implements the shared pnpm + uv global store (Feature 4).

pub mod cli;
pub mod hub;
pub mod project;
pub mod report;
pub mod run;
pub mod scan;
pub mod tui;
pub mod util;
