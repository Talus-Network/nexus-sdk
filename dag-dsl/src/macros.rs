//! `tool_descriptor!` declarative macro — the no-codegen, no-fetch, no-run
//! way to produce a [`crate::ToolDescriptor`] implementation from an inline
//! port specification.
//!
//! Example:
//!
//! ```
//! nexus_dag_dsl::tool_descriptor! {
//!     pub struct AddTool;
//!     fqn = "xyz.taluslabs.math.i64.add@1";
//!     inputs {
//!         a: i64,
//!         b: i64,
//!     }
//!     outputs {
//!         ok {
//!             result: i64,
//!         }
//!     }
//! }
//!
//! use nexus_dag_dsl::{ToolDescriptor, TypedDagBuilder};
//! let mut dag = TypedDagBuilder::new();
//! let a = dag.add::<AddTool>("a");
//! // a.inp.a, a.inp.b are InPort<i64>
//! // a.out.ok.result is OutPort<Ok, i64>
//! # let _ = a;
//! ```
//!
//! Tool authors who want the strict DSL layer can produce descriptors
//! without depending on a proc-macro, without fetching metadata, and
//! without running codegen. The alternative production paths (a
//! `nexus-toolkit` `#[derive(NexusToolDescriptor)]` — not implemented in
//! this iteration, tracked separately — and `nexus-dsl-codegen` reading
//! `tool-meta.json` artifacts) layer on top of the same trait surface.

/// Generate a [`crate::ToolDescriptor`] impl for the given tool type,
/// with strongly-typed `Inputs`/`Outputs` structs built from the port
/// spec.
///
/// Note: `paste` is re-exported from this crate and used internally for
/// identifier concatenation. Callers don't need to depend on `paste`.
///
/// Multiple output variants are supported; each variant produces its own
/// per-variant struct (e.g. `<Tool>Ok`, `<Tool>Err`) and appears as a
/// field of `<Tool>Outputs`.
#[macro_export]
macro_rules! tool_descriptor {
    (
        $(#[$struct_meta:meta])*
        $vis:vis struct $tool:ident;
        fqn = $fqn:literal;
        inputs {
            $( $in_name:ident : $in_ty:ty ),* $(,)?
        }
        outputs {
            $(
                $variant:ident {
                    $( $out_name:ident : $out_ty:ty ),* $(,)?
                }
            )*
        }
    ) => {
        $(#[$struct_meta])*
        $vis struct $tool;

        $crate::__paste::paste! {
            #[doc = "Typed input-port view for [`" $tool "`]."]
            $vis struct [<$tool Inputs>] {
                $( $vis $in_name: $crate::InPort<$in_ty>, )*
            }

            #[doc = "Typed outputs view for [`" $tool "`] — one field per variant."]
            $vis struct [<$tool Outputs>] {
                $( $vis $variant: [<$tool $variant:camel>], )*
            }

            $(
                #[doc = "Typed port view for the `" $variant "` variant of [`" $tool "`]."]
                $vis struct [<$tool $variant:camel>] {
                    $( $vis $out_name: $crate::OutPort<$crate::Ok, $out_ty>, )*
                }
            )*

            impl $crate::ToolDescriptor for $tool {
                type Inputs = [<$tool Inputs>];
                type Outputs = [<$tool Outputs>];

                fn fqn() -> $crate::ToolFqn {
                    <$crate::ToolFqn as ::core::str::FromStr>::from_str($fqn)
                        .expect(concat!("tool_descriptor! FQN must be valid: ", $fqn))
                }

                fn inputs_for(vertex_name: &str) -> Self::Inputs {
                    [<$tool Inputs>] {
                        $( $in_name: $crate::InPort::new(
                            vertex_name,
                            stringify!($in_name),
                        ), )*
                    }
                }

                fn outputs_for(vertex_name: &str) -> Self::Outputs {
                    [<$tool Outputs>] {
                        $(
                            $variant: [<$tool $variant:camel>] {
                                $(
                                    $out_name: $crate::OutPort::new(
                                        vertex_name,
                                        stringify!($variant),
                                        stringify!($out_name),
                                    ),
                                )*
                            },
                        )*
                    }
                }
            }
        }
    };
}
