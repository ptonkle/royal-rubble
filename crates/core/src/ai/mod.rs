//! Bot intelligence on top of the physics and the match terrain.
//!
//! - `aim`: per shot, a release tick and maybe an air effect, found by
//!   flying candidates and playing the best out on a clone of the world.
//! - `build`: castle plans from templates, scaled to level, gold and span,
//!   that stand up under the static test.
//! - `units`: the one place terrain, rules and physics units meet.
//! - `tune`: every number.

pub mod aim;
pub mod build;
pub mod planner;
pub mod tune;
pub mod units;

#[cfg(test)]
mod tests;
