//! Snowfall's snow engine, free of any platform: the simulation, its drifts and the geometry they use.

pub mod geometry;
pub mod layer;
pub mod simulation;
pub mod version;

#[cfg(test)]
mod tests;
