//! tool_core: every application rule of Royal Rubble.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.
//!
//! Keep `api.rs` as the one door the shim calls through.

pub mod api;
pub mod app;
pub mod game;
pub mod glossary;
pub mod icon;
pub mod lobby;
pub mod online;
pub mod page;
pub mod patch_notes;
pub mod physics;
pub mod rng;
pub mod save;
pub mod strings;
pub mod terrain;
pub mod ticks;
pub mod view;
