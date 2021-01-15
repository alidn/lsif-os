use std::{
    path::PathBuf,
    sync::mpsc::{channel, Sender},
};

use languageserver_types::NumberOrString;
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
            "/Users/zas/space/lsif-os/src/tests/test_data/{}",
            lang.to_string()
        )),
        language: lang,
        output: None,
    };

    Indexer::index(opts, emitter).unwrap();

    rx.recv().unwrap()
}

pub fn find_range(
    elements: &Elements,
    filename: &str,
    line_char: (u64, u64),
) -> Option<(Range, ID)> {
    for (v, id) in elements.vertices() {
        match v {
            Vertex::Range(r) => {
                if r.start.line == line_char.0 && r.start.character == line_char.1 {
                    if &find_document_uri_containing(elements, id)? == filename {
                        return Some((r.clone(), id));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_range_by_id(elements: &Elements, target_id: ID) -> Option<Range> {
    for (v, id) in elements.vertices() {
        if let Vertex::Range(r) = v {
            if id == target_id {
                return Some(r.clone());
            }
        }
    }

    None
}

pub fn find_definition_ranges(elements: &Elements, id: ID) -> Vec<Range> {
    let mut ranges = Vec::new();
    for (e, _) in elements.edges() {
        if let Edge::Definition(def) = e {
            if to_number(&def.out_v) == id {
                ranges.extend(find_definition_ranges_by_result_id(
                    &elements,
                    to_number(&def.in_v),
                ));
            }
        }
    }

    for (e, _) in elements.edges() {
        if let Edge::Next(def) = e {
            if to_number(&def.out_v) == id {
                ranges.extend(find_definition_ranges(&elements, to_number(&def.in_v)));
            }
        }
    }

    ranges
}

fn find_definition_ranges_by_result_id(elements: &Elements, id: ID) -> Vec<Range> {
    let mut ranges = Vec::new();
    for (e, _) in elements.edges() {
        if let Edge::Item(item) = e {
            let d = match &item {
                protocol::types::Item::Definition(v) => v,
                protocol::types::Item::Reference(v) => v,
                protocol::types::Item::Neither(v) => v,
            };
            for in_v in &d.in_vs {
                if let Some(range) = find_range_by_id(&elements, to_number(in_v)) {
                    ranges.push(range);
                }
            }
        }
    }
    ranges
}

pub fn find_document_uri_containing(elements: &Elements, id: ID) -> Option<String> {
    for (e, _) in elements.edges() {
        match e {
            Edge::Contains(d) => {
                for in_v in &d.in_vs {
                    if to_number(in_v) == id {
                        return find_uri_by_document_id(elements, to_number(&d.out_v));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn find_uri_by_document_id(elements: &Elements, target_id: ID) -> Option<String> {
    for (v, id) in elements.vertices() {
        match &v {
            Vertex::Document(d) => {
                if id == target_id {
                    return Some(d.uri.to_string());
                }
            }
            _ => {}
        }
    }

    None
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
