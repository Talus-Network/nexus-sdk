//! Extension methods for generated Move binding types.
//!
//! The generated ABI types remain under [`crate::move_bindings`]. These private modules attach
//! SDK domain behavior to those generated values without making [`crate::types`] another ABI
//! namespace. Code here should decode generated data, derive SDK views from it, or construct
//! values that preserve the generated BCS layout.

mod leader_registry;
mod network_auth;
mod nexus_data;
mod payment;
mod ports_data;
mod runtime_vertex;
mod scheduler_models;
mod shared_object_ref;
mod support;
mod tap;
mod workflow;
