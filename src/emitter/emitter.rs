use crate::protocol::types::{Edge, Vertex, ID};

/// An abstractions over an LSIF dump emitter.
pub trait Emitter {
    fn emit_vertex<V: Into<Vertex>>(&mut self, v: V) -> ID;

    fn emit_edge<E: Into<Edge>>(&mut self, e: E) -> ID;

    fn end(&mut self);
}
