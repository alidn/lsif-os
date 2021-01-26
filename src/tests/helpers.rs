use std::{
    path::PathBuf,
    sync::mpsc::{channel, Sender},
};

use languageserver_types::{NumberOrString, Url};
use protocol::types::Range;

use crate::{
    cli::Opts,
    emitter::emitter::Emitter,
    indexer::indexer::Indexer,
    protocol::{
        self,
        types::{Edge, Element, Language, Vertex, ID},
    },
};

/// Returns the absolute path to the root directory of this repository as a `String`
pub fn project_root() -> String {
    let root = std::fs::canonicalize(PathBuf::from("."))
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    root
}

/// Returns the absolute path to the root directory of this repository as a `Url`
pub fn project_root_uri() -> Url {
    protocol::types::Url::from_file_path(project_root()).unwrap()
}

/// Indexes the test data of the given language and returns the LSIF elements found.
/// Each LSIF element corresponds to a line emitted in an LSIF dump.
pub fn get_elements(lang: Language) -> Elements {
    let (tx, rx) = channel();
    let emitter = TestsEmitter {
        elements: Default::default(),
        tx,
        id: 0,
    };

    let opts = Opts {
        project_root: PathBuf::from(format!(
            "{}/src/tests/test_data/{}",
            project_root(),
            lang.to_string()
        )),
        language: lang,
        output: None,
    };

    Indexer::index(opts, emitter).unwrap();

    rx.recv().unwrap()
}

impl Elements {
    /// Returns the range in the given file with the given start line and character.
    pub fn find_range(&self, filename: &str, line_char: (u64, u64)) -> Option<(Range, ID)> {
        for (v, id) in self.vertices() {
            if let Vertex::Range(r) = v {
                if r.start.line == line_char.0 && r.start.character == line_char.1 {
                    if &self.find_document_uri_containing(id)? == filename {
                        return Some((r.clone(), id));
                    }
                }
            }
        }
        None
    }

    fn find_range_by_id(&self, target_id: ID) -> Option<Range> {
        for (v, id) in self.vertices() {
            if let Vertex::Range(r) = v {
                if id == target_id {
                    return Some(r.clone());
                }
            }
        }

        None
    }

    /// Returns the definition ranges attached to the range or result set
    /// with the given identifier.
    pub fn find_definition_ranges(&self, id: ID) -> Vec<Range> {
        let mut ranges = Vec::new();
        for (e, _) in self.edges() {
            if let Edge::Definition(def) = e {
                if to_number(&def.out_v) == id {
                    ranges.extend(self.find_definition_ranges_by_result_id(to_number(&def.in_v)));
                }
            }
        }

        for (e, _) in self.edges() {
            if let Edge::Next(def) = e {
                if to_number(&def.out_v) == id {
                    ranges.extend(self.find_definition_ranges(to_number(&def.in_v)));
                }
            }
        }

        ranges
    }

    /// Returns the ranges attached to the definition result with the given
    /// identifier.
    fn find_definition_ranges_by_result_id(&self, id: ID) -> Vec<Range> {
        let mut ranges = Vec::new();
        for (e, _) in self.edges() {
            if let Edge::Item(item) = e {
                let edge = match &item {
                    protocol::types::Item::Definition(v) => v,
                    protocol::types::Item::Reference(v) => v,
                    protocol::types::Item::Neither(v) => v,
                };
                if to_number(&edge.out_v) == id {
                    for in_v in &edge.in_vs {
                        if let Some(range) = self.find_range_by_id(to_number(in_v)) {
                            ranges.push(range);
                        }
                    }
                }
            }
        }
        ranges
    }

    /// Returns the URI of the document that contains the vertex with the given id.
    pub fn find_document_uri_containing(&self, id: ID) -> Option<String> {
        for (e, _) in self.edges() {
            if let Edge::Contains(d) = e {
                for in_v in &d.in_vs {
                    if to_number(in_v) == id {
                        return self.find_uri_by_document_id(to_number(&d.out_v));
                    }
                }
            }
        }
        None
    }

    /// Returns the URI of the document with the given id.
    pub fn find_uri_by_document_id(&self, target_id: ID) -> Option<String> {
        for (v, id) in self.vertices() {
            if let Vertex::Document(d) = v {
                if id == target_id {
                    return Some(d.uri.to_string());
                }
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct Elements(Vec<Entry>);

#[derive(Clone, Debug)]
struct Entry {
    id: ID,
    element: Element,
}

impl Elements {
    fn vertices(&self) -> Vec<(&Vertex, ID)> {
        self.0
            .iter()
            .filter_map(|e| match &e.element {
                Element::Vertex(v) => Some((v, e.id)),
                Element::Edge(_) => None,
            })
            .collect()
    }

    fn edges(&self) -> Vec<(&Edge, ID)> {
        self.0
            .iter()
            .filter_map(|e| match &e.element {
                Element::Vertex(_) => None,
                Element::Edge(v) => Some((v, e.id)),
            })
            .collect()
    }
}

fn to_number(n: &NumberOrString) -> ID {
    match n {
        NumberOrString::Number(n) => *n,
        NumberOrString::String(_) => panic!(),
    }
}

/// An implementation of `Emitter` used in tests.
pub struct TestsEmitter {
    elements: Vec<Entry>,
    pub tx: Sender<Elements>,

    id: ID,
}

impl TestsEmitter {
    fn next_id(&mut self) -> ID {
        self.id += 1;
        self.id
    }
}

impl Emitter for TestsEmitter {
    fn emit_edge<E: Into<Edge>>(&mut self, e: E) -> ID {
        let id = self.next_id();
        let edge = e.into();
        let entry = Entry {
            id,
            element: Element::Edge(edge),
        };
        self.elements.push(entry);
        id
    }

    fn emit_vertex<V: Into<Vertex>>(&mut self, v: V) -> ID {
        let id = self.next_id();
        let vertex = v.into();
        let entry = Entry {
            id,
            element: Element::Vertex(vertex),
        };
        self.elements.push(entry);
        id
    }

    fn end(&mut self) {
        self.tx.send(Elements(self.elements.clone())).unwrap();
    }
}
