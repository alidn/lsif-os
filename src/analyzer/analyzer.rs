use std::{
    collections::HashMap,
    mem::take,
    path::PathBuf,
    sync::{mpsc::Sender, Arc},
};

use anyhow::Context;
use smol_str::SmolStr;
use tree_sitter::{Node, Point, Query, QueryCursor, QueryMatch, Range, Tree};

use crate::protocol::types as protocol;

pub struct Analyzer<'sender> {
    /// The name of the file that is analysed.
    filename: String,

    /// The sending half of the channel for sending found definition during the analysis.
    def_sender: &'sender Sender<Arc<Definition>>,
    /// The sending half of the channel for references found during the analysis.
    reference_sender: &'sender Sender<Reference>,

    // Analysis cache:
    /// The last comment that was found during the AST walk.
    last_comment: Option<String>,
    /// Cache of all the definitions (name -> List of definition with that name).
    defs: HashMap<SmolStr, Vec<Arc<Definition>>>,
    /// Cache of all the references.
    refs: Vec<Reference>,
    /// Cache of scopes.
    /// NOTE: Using 'Vec' instead of a HashMap to store and lookup scopes might seem inefficient, but it's not.
    /// Because scopes are stored in the same order they are defined.
    scopes: Vec<Scope>,
    /// The content of the file in bytes.
    file_content_bytes: &'sender [u8],
}

impl<'sender> Analyzer<'sender> {
    /// Runs the analysis on the given file, sends the found definitions and references
    /// via the given channels.
    pub fn run_analysis(
        filename: String,
        tree: &Tree,
        query: &Query,
        def_sender: &'sender Sender<Arc<Definition>>,
        ref_sender: &'sender Sender<Reference>,
        file_content: &'sender String,
        query_names: &Vec<String>,
    ) {
        let mut analyzer = Self {
            def_sender,
            reference_sender: ref_sender,
            filename,
            file_content_bytes: file_content.as_bytes(),
            last_comment: None,
            defs: Default::default(),
            refs: Default::default(),
            scopes: Default::default(),
        };

        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor
            .matches(query, tree.root_node(), |_| [])
            .map(|m| (&query_names[m.pattern_index], m));
        for (name, qmatch) in matches {
            match analyzer.data_from_query_match(qmatch, name) {
                AnalysisData::Definition(it) => analyzer.handle_definition(Arc::new(it)),
                AnalysisData::Scope(it) => analyzer.cache_scope(it),
                AnalysisData::Comment(it) => analyzer.cache_comment(it),
                AnalysisData::Reference(mut it) => {
                    analyzer.try_find_def_of(&mut it);
                    analyzer.refs.push(it)
                }
            }
        }

        let mut refs = take(&mut analyzer.refs);
        analyzer.try_link_references(&mut refs);
        refs.into_iter()
            .for_each(|r| analyzer.reference_sender.send(r).unwrap());
    }

    /// Gets a query match found by treesitter and returns the `AnalysisData` extracted from it.
    fn data_from_query_match(&mut self, qmatch: QueryMatch<'sender>, query: &str) -> AnalysisData {
        use AnalysisData::*;

        match query {
            "definition.scoped" => {
                let def = self.definition_from(qmatch, true);
                Definition(def)
            }
            "definition.exported" => {
                let def = self.definition_from(qmatch, false);
                Definition(def)
            }
            "comment" => {
                let comment = self.comment_from(qmatch);
                Comment(comment)
            }
            "scope" => {
                let scope = self.scope_from(qmatch);
                Scope(scope)
            }
            "reference" => {
                let r = self.reference_from(qmatch);
                Reference(r)
            }
            _ => panic!("Unknown query {}", query),
        }
    }

    /// Caches the definition and sends it in the channel.
    fn handle_definition(&mut self, def: Arc<Definition>) {
        self.cache_definition(Arc::clone(&def));
        self.def_sender.send(def).unwrap();
    }

    /// Tries to find a definition for each of the given references. If a definition is not found,
    /// it means it is located in a different file or in a dependency library.
    fn try_link_references(&mut self, refs: &mut Vec<Reference>) {
        for mut r in refs {
            if !r.has_def() {
                self.try_find_def_of(&mut r);
            }
        }
    }
}

/// Represents data found (extracted) from a treesitter query match.
enum AnalysisData {
    Definition(Definition),
    Scope(Scope),
    Comment(String),
    Reference(Reference),
}

/// Methods for caching and retrieving analysis data.
impl<'a> Analyzer<'a> {
    fn cache_definition(&mut self, def: Arc<Definition>) {
        self.defs
            .entry(SmolStr::clone(&def.node_name))
            .or_default()
            .push(def);
    }

    fn cache_scope(&mut self, scope: Scope) {
        self.scopes.push(scope);
    }

    /// Finds the innermost scope that contains the given range.
    fn find_enclosing_scope(&self, range: &Range) -> Option<Scope> {
        self.scopes.iter().rev().find_map(|s| {
            if s.range.contains(range) {
                Some(*s)
            } else {
                None
            }
        })
    }

    /// If the last comment if not `None`, it replaces it with `None` and returns the comment,
    /// Otherwise, returns `None`.
    fn use_last_comment(&mut self) -> Option<String> {
        let mut ret = None;
        std::mem::swap(&mut ret, &mut self.last_comment);
        ret
    }

    /// Sets the value of the last comment to the given comment.
    fn cache_comment(&mut self, comment: String) {
        self.last_comment = Some(comment);
    }

    /// If the given reference already has a definition, does nothing. Otherwise, looks-up
    /// all the definition that are visible from the scope of the give reference. If it finds
    /// a definition that matches that reference's name, it sets its definition value.
    fn try_find_def_of(&self, r: &mut Reference) {
        r.def = self.defs.get(&r.node_name).and_then(|l| {
            l.iter()
                .rev()
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
}

impl<'a> Analyzer<'a> {
    /// Returns a `Scope` from the given query match. It is the reponsibility
    /// of the caller to ensure that the query match is the result
    /// of a 'scope' query.
    fn scope_from(&mut self, qmatch: QueryMatch) -> Scope {
        Scope {
            range: qmatch.captures[0].node.range(),
        }
    }

    /// Returns a `Comment` from the given query match. It is the reponsibility
    /// of the caller to ensure that the query match is the result
    /// of a 'comment' query.
    fn comment_from(&mut self, qmatch: QueryMatch) -> String {
        self.node_text_of(&qmatch.captures[0].node)
    }

    /// Returns a `Reference` from the given query match. It is the reponsibility
    /// of the caller to ensure that the query match is the result
    /// of a 'reference' query.
    fn reference_from(&mut self, qmatch: QueryMatch) -> Reference {
        let capture = qmatch.captures[0];
        let name = SmolStr::new(self.node_text_of(&capture.node));
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

        Reference {
            location: self.location_of(&capture.node),
            node_name: name,
            def,
        }
    }

    /// Returns a `Definition` from the given query match. It is the reponsibility
    /// of the caller to ensure that the query match is the result
    /// of a 'definition' query.
    fn definition_from(&mut self, qmatch: QueryMatch, scoped: bool) -> Definition {
        let capture = qmatch.captures[0];
        let kind = if scoped {
            DefinitionKind::Scoped(
                self.find_enclosing_scope(&capture.node.range())
                    .context(format!(
                        "Expected node at (file: {}, line: {}, column: {}) to have a scope\n
                        This error probably means that the query file is wrong.",
                        self.filename,
                        capture.node.range().start_point.row + 1,
                        capture.node.range().start_point.column + 1
                    ))
                    .unwrap_or(Scope {
                        range: Range {
                            start_byte: 0,
                            end_byte: 0,
                            start_point: Point { row: 0, column: 0 },
                            end_point: Point { row: 0, column: 0 },
                        },
                    })
                    .range,
            )
        } else {
            DefinitionKind::Exported
        };

        Definition {
            location: self.location_of(&capture.node),
            node_name: SmolStr::new(self.node_text_of(&capture.node)),
            comment: Some(
                self.use_last_comment()
                    .unwrap_or(self.line_of(&capture.node)),
            ),
            kind,
        }
    }

    /// Returns the `Location` of the given node.
    fn location_of(&self, node: &Node) -> Location {
        Location {
            file_path: self.filename.clone(),
            range: node.range(),
        }
    }

    /// Returns the name content of the given node as a String
    fn node_text_of(&self, node: &Node) -> String {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        std::str::from_utf8(&self.file_content_bytes[start_byte..end_byte])
            .unwrap()
            .to_string()
    }

    /// Returns the text of the line where the first start of the node is located. This is
    /// used for hover contents when a variable is not documented.
    fn line_of(&self, node: &Node) -> String {
        let start_byte = node.start_byte();
        let end_byte = self.file_content_bytes[start_byte..]
            .iter()
            .enumerate()
            .find(|(_i, c)| c == &&b'\n')
            .map(|(i, _c)| i + start_byte)
            .unwrap_or(start_byte);
        format!(
            "{} {}",
            node.kind().to_string(),
            std::str::from_utf8(&self.file_content_bytes[start_byte..end_byte])
                .unwrap()
                .to_string()
        )
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

#[derive(Debug, Clone)]
pub struct Reference {
    pub location: Location,
    pub node_name: SmolStr,
    pub def: Option<Arc<Definition>>,
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

impl Reference {
    fn has_def(&self) -> bool {
        self.def.is_some()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Location {
    pub range: Range,
    pub file_path: String,
}

impl Location {
    /// Returns the name of the file (the final component of the file path)
    pub fn file_name(&self) -> String {
        PathBuf::from(&self.file_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }
}

pub trait Contain {
    fn contains(&self, o: &Self) -> bool;
}

impl Contain for Range {
    fn contains(&self, o: &Self) -> bool {
        self.end_byte >= o.end_byte && self.start_byte <= o.start_byte
    }
}
