use crate::protocol::types::{Edge, Vertex, ID};

/// An abstractions for an LSIF data emitter.
pub trait Emitter {
    fn emit_vertex<V: Into<Vertex>>(&mut self, v: V) -> ID;

    fn emit_edge<E: Into<Edge>>(&mut self, e: E) -> ID;

    /// This method needs to be called to ensure that all items
    /// have been emitted.
    fn end(&mut self);
}
