use std::{
    collections::HashMap,
    sync::{mpsc::Sender, Arc},
};

use anyhow::Context;
use smol_str::SmolStr;
use tree_sitter::{Node, Point, Query, QueryCursor, QueryMatch, Range, Tree};

use crate::protocol::types as protocol;

pub struct Analyzer<'sender> {
    def_sender: &'sender Sender<Arc<Definition>>,
    reference_sender: &'sender Sender<Reference>,

    filename: String,
    file_content_bytes: &'sender [u8],

    last_comment: Option<String>,

    defs: HashMap<SmolStr, Vec<Arc<Definition>>>,
    refs: Vec<Reference>,
    // NOTE: Using 'Vec' instead of a HasHashmap to store and lookup scopes might seem inefficient, but it's not.
    // Because scopes are stored in the same order they are defined.
    scopes: Vec<Scope>,
}

impl<'sender> Analyzer<'sender> {
    pub fn run_analysis(
        filename: String,
        tree: &Tree,
        query: &Query,
        def_sender: &'sender Sender<Arc<Definition>>,
        ref_sender: &'sender Sender<Reference>,
        file_content: &'sender String,
        query_names: &Vec<String>,
    ) {
        let mut query_cursor = QueryCursor::new();

        let query_matches = query_cursor.matches(query, tree.root_node(), |_| []);

        let mut s = Self {
            def_sender,
            reference_sender: ref_sender,
            filename,
            file_content_bytes: file_content.as_bytes(),
            last_comment: None,
            defs: Default::default(),
            refs: Default::default(),
            scopes: Default::default(),
        };

        for q in query_matches {
            let query_name = &query_names[q.pattern_index];
            s.handle_match(q, query_name);
        }

        let refs = std::mem::take(&mut s.refs);
        for mut ref_match in refs {
            if ref_match.def.is_some() {
                s.reference_sender.send(ref_match).unwrap();
            } else {
                s.find_def_for(&mut ref_match);
                s.reference_sender.send(ref_match).unwrap();
            }
        }
    }

    fn handle_definition(&mut self, def: Arc<Definition>) {
        let defs_with_name = self.defs.entry(SmolStr::clone(&def.node_name)).or_default();
        defs_with_name.push(Arc::clone(&def));

        self.def_sender.send(def).unwrap();
    }

    fn handle_scope(&mut self, scope: Scope) {
        self.scopes.push(scope);
    }

    fn handle_match(&mut self, qmatch: QueryMatch<'sender>, query: &str) {
        match query {
            "definition.scoped" => {
                let def = self.definition_from(qmatch, true);
                self.handle_definition(Arc::new(def));
            }
            "definition.exported" => {
                let def = self.definition_from(qmatch, false);
                self.handle_definition(Arc::new(def));
            }
            "comment" => {
                let comment = self.comment_from(qmatch);
                self.set_last_comment(comment);
            }
            "scope" => {
                let scope = self.scope_from(qmatch);
                self.handle_scope(scope);
            }
            "reference" => {
                if let Some(r) = self.reference_from(qmatch) {
                    self.refs.push(r);
                }
            }
            _ => panic!("Unknown query {}", query),
        };
    }

    fn find_enclosing_scope(&self, range: &Range) -> Option<Scope> {
        for s in self.scopes.iter().rev() {
            if s.range.contains(range) {
                return Some(*s);
            }
        }
        None
    }

    /// If the last comment if not `None`, it replaces it with `None` and returns the comment,
    /// Otherwise, returns `None`.
    fn use_last_comment(&mut self) -> Option<String> {
        let mut ret = None;
        std::mem::swap(&mut ret, &mut self.last_comment);
        ret
    }

    /// Sets the value of the last comment to the given comment.
    fn set_last_comment(&mut self, comment: String) {
        self.last_comment = Some(comment);
    }
}

impl<'a> Analyzer<'a> {
    fn scope_from(&mut self, qmatch: QueryMatch) -> Scope {
        Scope {
            range: qmatch.captures[0].node.range(),
        }
    }

    fn comment_from(&mut self, qmatch: QueryMatch) -> String {
        self.node_text(&qmatch.captures[0].node)
    }

    fn find_def_for(&self, r: &mut Reference) {
        r.def = self.defs.get(&r.node_name).and_then(|l| {
            l.iter()
                .find(|&d| {
                    let matches_name = d.node_name == r.node_name;
                    let is_in_scope = match &d.kind {
                        DefinitionKind::Exported => true,
                        DefinitionKind::Scoped(scope) => scope.contains(&r.location.range),
                    };

                    matches_name && is_in_scope
                })
                .map(Arc::clone)
        })
    }

    fn reference_from(&mut self, qmatch: QueryMatch) -> Option<Reference> {
        let capture = qmatch.captures[0];
        let name = SmolStr::new(self.node_text(&capture.node));
        let range = capture.node.range();

        let def = self
            .defs
            .entry(SmolStr::clone(&name))
            .or_default()
            .iter()
            .find(|&d| {
                let is_in_scope = match &d.kind {
                    DefinitionKind::Exported => true,
                    DefinitionKind::Scoped(scope) => scope.contains(&range),
                };

                d.location.range != range && is_in_scope
            })
            .map(Arc::clone);

        Some(Reference {
            location: self.node_location(&capture.node),
            node_name: name,
            def,
        })
    }

    fn definition_from(&mut self, qmatch: QueryMatch, scoped: bool) -> Definition {
        let capture = qmatch.captures[0];
        let kind = if scoped {
            DefinitionKind::Scoped(
                self.find_enclosing_scope(&capture.node.range())
                    .context(format!(
                        "Expected node at (line: {}, column: {}) to have a scope\n
                        This error probably means that the query file is wrong.",
                        capture.node.range().start_point.row + 1,
                        capture.node.range().start_point.column + 1
                    ))
                    .unwrap()
                    .range,
            )
        } else {
            DefinitionKind::Exported
        };

        Definition {
            location: self.node_location(&capture.node),
            node_name: SmolStr::new(self.node_text(&capture.node)),
            comment: self.use_last_comment(),
            kind,
        }
    }

    fn node_location(&self, node: &Node) -> Location {
        Location {
            filename: self.filename.clone(),
            range: node.range(),
        }
    }

    /// Returns the name content of the given node as a String
    fn node_text(&self, node: &Node) -> String {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        std::str::from_utf8(&self.file_content_bytes[start_byte..end_byte])
            .unwrap()
            .to_string()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Scope {
    range: Range,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub location: Location,
    pub node_name: SmolStr,
    pub comment: Option<String>,
    pub kind: DefinitionKind,
}

impl Definition {
    pub fn range(&self) -> protocol::Range {
        protocol::Range {
            start: protocol::Position::from_point(self.location.range.start_point),
            end: protocol::Position::from_point(self.location.range.end_point),
        }
    }
}

impl Reference {
    pub fn range(&self) -> protocol::Range {
        protocol::Range {
            start: protocol::Position::from_point(self.location.range.start_point),
            end: protocol::Position::from_point(self.location.range.end_point),
        }
    }
}

trait FromPoint {
    fn from_point(p: Point) -> Self;
}

impl FromPoint for protocol::Position {
    fn from_point(p: Point) -> Self {
        protocol::Position {
            line: p.row as u64,
            character: p.column as u64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionKind {
    Exported,
    Scoped(Range),
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub location: Location,
    pub node_name: SmolStr,
    pub def: Option<Arc<Definition>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Location {
    pub range: Range,
    pub filename: String,
}

pub trait Contain {
    fn contains(&self, o: &Self) -> bool;
}

impl Contain for Range {
    fn contains(&self, o: &Self) -> bool {
        self.end_byte >= o.end_byte && self.start_byte <= o.start_byte
    }
}
