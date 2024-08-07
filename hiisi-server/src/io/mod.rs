#[cfg(not(feature = "simulation"))]
mod generic;

#[cfg(not(feature = "simulation"))]
pub use generic::IO;

#[cfg(feature = "simulation")]
mod simulation;

#[cfg(feature = "simulation")]
pub use simulation::IO;
