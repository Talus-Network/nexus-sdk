//! SDK helpers for generated `nexus_interface::graph::RuntimeVertex`.

use crate::move_bindings::{
    interface::graph::{RuntimeVertex, Vertex},
    move_std::{ascii::String as MoveString, type_name::TypeName},
};

impl RuntimeVertex {
    pub fn plain(vertex: &str) -> Self {
        Self::Plain {
            vertex: vertex_from_str(vertex),
        }
    }

    pub fn with_iterator(vertex: &str, iteration: u64, out_of: u64) -> Self {
        Self::WithIterator {
            vertex: vertex_from_str(vertex),
            iteration,
            out_of,
        }
    }

    pub fn vertex(&self) -> &Vertex {
        match self {
            Self::Plain { vertex } | Self::WithIterator { vertex, .. } => vertex,
        }
    }

    pub fn vertex_name(&self) -> &str {
        self.vertex().as_str()
    }

    pub fn name(&self) -> TypeName {
        TypeName::from(self.vertex_name())
    }
}

impl std::fmt::Display for RuntimeVertex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain { vertex } => write!(f, "Plain({})", vertex.name.as_str()),
            Self::WithIterator {
                vertex,
                iteration,
                out_of,
            } => write!(
                f,
                "WithIterator({}:{}:{})",
                vertex.name.as_str(),
                iteration,
                out_of
            ),
        }
    }
}

fn vertex_from_str(vertex: &str) -> Vertex {
    Vertex::new(vertex)
}

impl From<TypeName> for Vertex {
    fn from(value: TypeName) -> Self {
        Self {
            name: MoveString::from(value.name),
        }
    }
}

impl From<&TypeName> for Vertex {
    fn from(value: &TypeName) -> Self {
        Self {
            name: MoveString::from(value.name.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_vertex_helpers_cover_plain_and_iterator_variants() {
        let plain = RuntimeVertex::plain("source");
        assert_eq!(plain.name(), TypeName::new("source"));
        assert_eq!(plain.to_string(), "Plain(source)");

        let with_iterator = RuntimeVertex::with_iterator("worker", 2, 5);
        assert_eq!(with_iterator.name(), TypeName::new("worker"));
        assert_eq!(with_iterator.to_string(), "WithIterator(worker:2:5)");
    }

    #[test]
    fn type_name_conversions_create_move_vertices() {
        let owned_name = TypeName::new("owned");
        let borrowed_name = TypeName::new("borrowed");

        let owned: Vertex = owned_name.into();
        let borrowed: Vertex = (&borrowed_name).into();

        assert_eq!(owned.name.as_str(), "owned");
        assert_eq!(borrowed.name.as_str(), "borrowed");
    }
}
