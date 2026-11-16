//! Snowfall's snow engine, free of any platform: the simulation, its Piles and the geometry they use.

pub mod geometry;
pub mod layer;
pub mod play;
pub mod plough;
pub mod simulation;

#[cfg(test)]
mod tests;
