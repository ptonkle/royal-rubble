//! Royal Rubble's own deterministic 2D physics, in profile view.
//!
//! Fixed-point (Q16.16) throughout, Vec and index order only, so the same
//! build and the same shots give the same bits on every machine. One tick is
//! 1/60 s. Every material and feel number lives in `tune::TUNE`.
//!
//! - `world`: nodes, segments, joints, the solver, strain and the static test.
//! - `balls`: shots and debris, air effects, fire.
//! - `bodies`: royals and buildings standing on floors; hit, crushed, fell.
//! - `trebuchet`: release tick and power to launch position and velocity.

pub mod balls;
pub mod bodies;
pub mod fixed;
pub mod trebuchet;
pub mod tune;
pub mod world;

pub use balls::{AirEffect, Ball};
pub use bodies::Body;
pub use fixed::{Fx, Vec2};
pub use trebuchet::{arm_at, trebuchet_launch, Launch};
pub use tune::{Material, TUNE};
pub use world::{BreakCause, BuildError, Event, Segment, StaticReport, Terrain, World};

#[cfg(test)]
mod tests;
