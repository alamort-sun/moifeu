// orb-weaver — Procedural Soft-Body Orb-Weaver Spider Protocol
// Wraps Rust simulation in Moifeu beam communication.
// The closed mechanical ring: body → legs → world → tension → body

pub mod beams; // Moifeu beam definitions for each simulation pass
pub mod creature; // Spider controller wiring all passes
pub mod gait_patterns; // Gait modes, transitions, personality coupling
pub mod spectrum; // Color field: pragonastatic ↔ sporagonastatic gradient trajectory

// Re-exports for crate consumers
pub use beams::creature_update_frame;
pub use creature::{PersonalityState, RenderState, Spider};
pub use gait_patterns::{GaitMode, GaitMode as GaitPattern, WebState};
pub use spectrum::ColorField;
