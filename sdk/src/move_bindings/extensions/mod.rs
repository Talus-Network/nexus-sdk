//! SDK-owned extension impls for generated Move bindings.
//!
//! The generated ABI types remain under `crate::move_bindings::*`; these private modules attach
//! Nexus-specific helpers to those generated types without making `crate::types` a second ABI
//! namespace.

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
