//! Wrapper around `nexus_workflow::dag::RuntimeVertex` type. This struct
//! contains the vertex name as [`crate::types::TypeName`] and the type of
//! the vertex.
//!
//! - [`RuntimeVertex::Plain`] only contains the vertex name.
//! - [`RuntimeVertex::WithIterator`] variant contains the data about
//!   which iteration of the vertex is being executed and what is the max number
//!   of iterations.

use {
    crate::types::*,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "@variant")]
pub enum RuntimeVertex {
    Plain {
        vertex: TypeName,
    },
    WithIterator {
        vertex: TypeName,
        #[serde(
            deserialize_with = "deserialize_sui_u64",
            serialize_with = "serialize_sui_u64"
        )]
        iteration: u64,
        #[serde(
            deserialize_with = "deserialize_sui_u64",
            serialize_with = "serialize_sui_u64"
        )]
        out_of: u64,
    },
}

impl RuntimeVertex {
    pub fn plain(vertex: &str) -> Self {
        RuntimeVertex::Plain {
            vertex: TypeName {
                name: vertex.to_string(),
            },
        }
    }

    pub fn with_iterator(vertex: &str, iteration: u64, out_of: u64) -> Self {
        RuntimeVertex::WithIterator {
            vertex: TypeName {
                name: vertex.to_string(),
            },
            iteration,
            out_of,
        }
    }

    pub fn name(&self) -> TypeName {
        match self {
            RuntimeVertex::Plain { vertex } => vertex.clone(),
            RuntimeVertex::WithIterator { vertex, .. } => vertex.clone(),
        }
    }
}

impl std::fmt::Display for RuntimeVertex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeVertex::Plain { vertex } => write!(f, "Plain({})", vertex.name),
            RuntimeVertex::WithIterator {
                vertex,
                iteration,
                out_of,
            } => write!(f, "WithIterator({}:{}:{})", vertex.name, iteration, out_of),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_plain() {
        let vertex = RuntimeVertex::Plain {
            vertex: TypeName {
                name: "vertex_a".to_string(),
            },
        };

        let json = serde_json::to_string(&vertex).unwrap();

        assert_eq!(json, r#"{"@variant":"Plain","vertex":{"name":"vertex_a"}}"#,);
    }

    #[test]
    fn test_deserialize_plain() {
        let json = r#"{
            "@variant": "Plain",
            "vertex": { "name": "vertex_b" }
        }"#;

        let vertex: RuntimeVertex = serde_json::from_str(json).unwrap();
        match vertex {
            RuntimeVertex::Plain { vertex } => {
                assert_eq!(
                    vertex,
                    TypeName {
                        name: "vertex_b".to_string()
                    }
                );
            }
            _ => panic!("Expected Plain variant"),
        }
    }

    #[test]
    fn test_serialize_with_iterator() {
        let vertex = RuntimeVertex::WithIterator {
            vertex: TypeName {
                name: "vertex_c".to_string(),
            },
            iteration: 5,
            out_of: 10,
        };

        let json = serde_json::to_string(&vertex).unwrap();

        assert_eq!(
            json,
            r#"{"@variant":"WithIterator","vertex":{"name":"vertex_c"},"iteration":"5","out_of":"10"}"#
        );
    }

    #[test]
    fn test_deserialize_with_iterator() {
        let json = r#"{
            "@variant": "WithIterator",
            "vertex": { "name": "vertex_d" },
            "iteration": "7",
            "out_of": "15"
        }"#;

        let vertex: RuntimeVertex = serde_json::from_str(json).unwrap();

        match vertex {
            RuntimeVertex::WithIterator {
                vertex,
                iteration,
                out_of,
            } => {
                assert_eq!(
                    vertex,
                    TypeName {
                        name: "vertex_d".to_string()
                    }
                );
                assert_eq!(iteration, 7);
                assert_eq!(out_of, 15);
            }
            _ => panic!("Expected WithIterator variant"),
        }
    }

    #[test]
    fn test_display_plain() {
        let vertex = RuntimeVertex::Plain {
            vertex: TypeName {
                name: "vertex_x".to_string(),
            },
        };
        let display = format!("{}", vertex);
        assert_eq!(display, "Plain(vertex_x)");
    }

    #[test]
    fn test_display_with_iterator() {
        let vertex = RuntimeVertex::WithIterator {
            vertex: TypeName {
                name: "vertex_y".to_string(),
            },
            iteration: 3,
            out_of: 8,
        };
        let display = format!("{}", vertex);
        assert_eq!(display, "WithIterator(vertex_y:3:8)");
    }

    #[test]
    fn test_plain_builder() {
        let vertex = RuntimeVertex::plain("builder_vertex");
        match vertex {
            RuntimeVertex::Plain { vertex } => {
                assert_eq!(
                    vertex,
                    TypeName {
                        name: "builder_vertex".to_string()
                    }
                );
            }
            _ => panic!("Expected Plain variant"),
        }
    }

    #[test]
    fn test_with_iterator_builder() {
        let vertex = RuntimeVertex::with_iterator("builder_vertex_iter", 2, 4);
        match vertex {
            RuntimeVertex::WithIterator {
                vertex,
                iteration,
                out_of,
            } => {
                assert_eq!(
                    vertex,
                    TypeName {
                        name: "builder_vertex_iter".to_string()
                    }
                );
                assert_eq!(iteration, 2);
                assert_eq!(out_of, 4);
            }
            _ => panic!("Expected WithIterator variant"),
        }
    }

    #[test]
    fn test_name_getter_plain() {
        let vertex = RuntimeVertex::Plain {
            vertex: TypeName {
                name: "plain_vertex".to_string(),
            },
        };
        let name = vertex.name();
        assert_eq!(
            name,
            TypeName {
                name: "plain_vertex".to_string()
            }
        );
    }

    #[test]
    fn test_name_getter_with_iterator() {
        let vertex = RuntimeVertex::WithIterator {
            vertex: TypeName {
                name: "iter_vertex".to_string(),
            },
            iteration: 1,
            out_of: 2,
        };
        let name = vertex.name();
        assert_eq!(
            name,
            TypeName {
                name: "iter_vertex".to_string()
            }
        );
    }

    #[test]
    fn test_equality_plain() {
        let v1 = RuntimeVertex::plain("eq_vertex");
        let v2 = RuntimeVertex::Plain {
            vertex: TypeName {
                name: "eq_vertex".to_string(),
            },
        };
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_equality_with_iterator() {
        let v1 = RuntimeVertex::with_iterator("eq_iter", 9, 99);
        let v2 = RuntimeVertex::WithIterator {
            vertex: TypeName {
                name: "eq_iter".to_string(),
            },
            iteration: 9,
            out_of: 99,
        };
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_inequality_between_variants() {
        let v1 = RuntimeVertex::plain("vertex");
        let v2 = RuntimeVertex::with_iterator("vertex", 0, 1);
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_debug_trait() {
        let v = RuntimeVertex::with_iterator("dbg", 1, 2);
        let dbg_str = format!("{:?}", v);
        assert!(dbg_str.contains("WithIterator"));
        assert!(dbg_str.contains("dbg"));
        assert!(dbg_str.contains("iteration: 1"));
        assert!(dbg_str.contains("out_of: 2"));
    }
}
