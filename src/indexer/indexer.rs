use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc::channel, Arc},
};

use anyhow::Result;
use ignore::{DirEntry, Walk};
use indicatif::{ProgressBar, ProgressStyle};
use languageserver_types::{NumberOrString, Url};
use rayon::prelude::*;
use tree_sitter::{Parser, Query, Tree};

use crate::{
    analyzer::{
        analyzer::{Analyzer, Definition, DefinitionScope, Reference},
        ffi::{parser_for_language, query_for_language, ts_language_from},
        file_utils::read_file,
        lsif_data_cache::{DefinitionInfo, LsifDataCache},
    },
    cli::Opts,
    edge,
    emitter::emitter::Emitter,
    protocol::types::{
        Contents, DefinitionResult, Document, Edge, EdgeData, HoverResult, LSIFMarkedString,
        Language, MetaData, Moniker, ReferenceResult, ResultSet, ToolInfo, ID,
    },
};

pub struct Indexer<E>
where
    E: Emitter,
{
    emitter: E,
    tool_info: ToolInfo,
    opt: Opts,

    project_id: ID,

    cache: LsifDataCache,

    cached_file_paths: Option<Vec<PathBuf>>,
}

impl<E> Indexer<E>
where
    E: Emitter,
{
    /// Generates an LSIF dump from a project by traversing through files of the given language
    /// and emitting the LSIF equivalent using the given emitter.
    pub fn index(opt: Opts, emitter: E) -> Result<()> {
        let mut indexer = Self {
            emitter,
            tool_info: ToolInfo::default(),
            opt: opt.clone(),
            project_id: 0,
            cache: LsifDataCache::default(),
            cached_file_paths: Default::default(),
        };

        indexer.emit_metadata_and_project_vertex();
        indexer.emit_documents();
        {
            let query = query_for_language(&opt.language)?;
            let files = indexer.file_paths();
            let files = parse_files(&opt.language, files)?;
            indexer.emit_definitions(files, &query);
        }
        indexer.link_reference_results_to_ranges();
        indexer.emit_contains();

        indexer.emitter.end();

        Ok(())
    }

    /// Emits the contains relationship for all documents and the ranges that they contain.
    fn emit_contains(&mut self) {
        let documents = self.cache.get_documents();
        for d in documents {
            let all_range_ids = [&d.reference_range_ids[..], &d.definition_range_ids[..]].concat();
            if !all_range_ids.is_empty() {
                self.emitter.emit_edge(Edge::contains(d.id, all_range_ids));
            }
        }
        self.emit_contains_for_project();
    }

    /// Emits a contains edge between a document and its ranges.
    fn emit_contains_for_project(&mut self) {
        let document_ids = self.cache.get_documents().map(|d| d.id).collect();
        self.emitter
            .emit_edge(Edge::contains(self.project_id, document_ids));
    }

    /// Emits item relations for each indexed definition result value.
    fn link_reference_results_to_ranges(&mut self) {
        let def_infos = self.cache.get_mut_def_infos();
        Self::link_items_to_definitions(&def_infos.collect(), &mut self.emitter);
    }

    /// Adds item relations between the given definition range and the ranges that
    /// define and reference it.
    fn link_items_to_definitions(def_infos: &Vec<&mut DefinitionInfo>, emitter: &mut E) {
        for d in def_infos {
            let ref_result_id = emitter.emit_vertex(ReferenceResult {});

            emitter.emit_edge(edge!(References, d.result_set_id -> ref_result_id));
            emitter.emit_edge(Edge::def_item(
                ref_result_id,
                vec![d.range_id],
                d.document_id,
            ));

            for (document_id, range_ids) in &d.reference_range_ids {
                emitter.emit_edge(Edge::ref_item(
                    ref_result_id,
                    range_ids.clone(),
                    *document_id,
                ));
            }
        }
    }

    fn emit_definitions(&mut self, files: HashMap<String, ParseResult>, query: &Query) {
        let (def_sender, def_receiver) = channel();
        let (ref_sender, ref_receiver) = channel();

        let capture_names = get_capture_names(&query, self.opt.language.get_query_source());

        let bar = ProgressBar::new(files.len() as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{bar:40.cyan/blue} {pos}/{len} files indexed")
                .progress_chars("==>"),
        );
        files.into_par_iter().for_each_with(
            (def_sender, ref_sender),
            |(d, r),
             (
                filename,
                ParseResult {
                    tree, file_content, ..
                },
            )| {
                Analyzer::run_analysis(filename, &tree, query, d, r, &file_content, &capture_names);
                bar.inc(1);
            },
        );

        for def in def_receiver {
            self.index_definition(def);
        }

        for r in ref_receiver {
            self.index_reference(r);
        }
        bar.finish_and_clear();
    }

    /// Emits data for the given reference object and caches it for emitting 'contains' later.
    fn index_reference(&mut self, r: Reference) {
        match &r.def {
            Some(def) => self.index_reference_to_definition(&def, &r),
            None => {
                if let Some(def) = self.cache.defs_with_name(&r.node_name).map(Arc::clone) {
                    self.index_reference_to_definition(&def, &r);
                } else {
                    // TODO: Find the definition which might be a dependency
                }
            }
        }
    }

    /// Returns a range identifier for the given reference. If a range for the object has
    /// not been emitted, a new vertex is created.
    fn ensure_range_for(&mut self, r: &Reference) -> ID {
        match self
            .cache
            .get_range_id(&r.location.file_path, r.location.range.start_byte)
        {
            Some(range_id) => range_id,
            None => {
                let range_id = self.emitter.emit_vertex(r.range());
                self.cache.cache_reference_range(r, range_id);
                range_id
            }
        }
    }

    /// Emits data for the given reference object that is defined within
    /// an index target package.
    fn index_reference_to_definition(&mut self, def: &Definition, r: &Reference) {
        // 1. Emit/Get vertices(s)
        let range_id = self.ensure_range_for(r);

        // 2. Connect the emitted vertices
        let next_edge = {
            let def_result_set_id = self
                .cache
                .get_definition_info(&def.location)
                .unwrap()
                .result_set_id;
            edge!(Next, range_id -> def_result_set_id)
        };
        self.emitter.emit_edge(next_edge);

        // 3. Cache the result
        self.cache.cache_reference(&def, &r, range_id);
    }

    /// Emits data for the given definition object and caches it for
    /// emitting 'contains' later.
    fn index_definition(&mut self, def: Arc<Definition>) {
        let document_id = self.cache.get_document_id(&def.location.file_path).unwrap();

        // 1. Emit Vertices
        let range_id = self.emitter.emit_vertex(def.range());
        let result_set_id = self.emitter.emit_vertex(ResultSet {});
        let def_result_id = self.emitter.emit_vertex(DefinitionResult {});
        let hover_result_id = self.emitter.emit_vertex(HoverResult {
            result: Contents {
                contents: vec![LSIFMarkedString {
                    language: self.opt.language.to_string(),
                    value: def.comment.clone(),
                    is_raw_string: true,
                }],
            },
        });
        let moniker_id = self.emitter.emit_vertex(Moniker {
            kind: if def.kind == DefinitionScope::Exported {
                "exported".to_string()
            } else {
                "local".to_string()
            },
            scheme: "zas".to_string(),
            identifier: format!("{}:{}", def.location.file_name(), def.node_name.clone()),
        });

        // 2. Connect the emitted vertices
        let next_edge = edge!(Next, range_id -> result_set_id);
        let definition_edge = edge!(Definition, result_set_id -> def_result_id);
        let item_edge = Edge::item(def_result_id, vec![range_id], document_id);
        let moniker_edge = edge!(Moniker, result_set_id -> moniker_id);
        let hover_edge = edge!(Hover, result_set_id -> hover_result_id);

        for edge in vec![
            next_edge,
            definition_edge,
            item_edge,
            moniker_edge,
            hover_edge,
        ]
        .into_iter()
        {
            self.emitter.emit_edge(edge);
        }

        // 3. Cache the result
        self.cache
            .cache_definition(&def, document_id, range_id, result_set_id);
    }

    /// Emits a metadata and project vertex. This method caches the identifier of the project
    /// vertex, which is needed to construct the project/document contains relation later.
    fn emit_metadata_and_project_vertex(&mut self) {
        self.project_id = self.emitter.emit_vertex(MetaData {
            version: "0.1".into(),
            position_encoding: "utf-16".into(),
            tool_info: Some(self.tool_info.clone()),
            project_root: Url::from_directory_path(&self.opt.project_root).unwrap(),
        });
    }

    fn emit_documents(&mut self) {
        self.file_paths().iter().for_each(|filename| {
            let document_id = self.emitter.emit_vertex(Document {
                uri: Url::from_file_path(&filename).unwrap(),
                language_id: self.opt.language,
            });
            self.cache
                .cache_document(filename.to_str().unwrap().to_string(), document_id);
        });
    }

    /// Returns a `Vec` of of paths of all the files that have the same format as this
    /// indexer's language.
    fn file_paths(&mut self) -> Vec<PathBuf> {
        if let Some(res) = &self.cached_file_paths {
            return res.clone();
        }

        let exs = self.opt.language.get_extensions();
        let res: Vec<PathBuf> = Walk::new(PathBuf::from(&self.opt.project_root))
            .into_iter()
            .filter_map(Result::ok)
            .filter(move |entry| {
                entry.metadata().unwrap().is_file() && check_extensions(entry, exs.clone())
            })
            .map(DirEntry::into_path)
            .collect();
        self.cached_file_paths = Some(res.clone());
        res
    }
}

/// Represents the result of parse operation on a file.
struct ParseResult {
    parser: Parser,
    tree: Tree,
    file_content: String,
}

/// Parses the given files with the given language's parser in parallel.
/// Returns a `HashMap` of filepath (as `String`) to `ParseResult`.
///
/// # Panics
/// Panics if it fails to parse a file.
fn parse_files(
    lang: &Language,
    files: Vec<PathBuf>,
) -> anyhow::Result<HashMap<String, ParseResult>> {
    let lang = ts_language_from(lang);
    let parsers = files
        .into_par_iter()
        .map(|path| {
            let mut parser = parser_for_language(lang).unwrap();
            let file_content = read_file(&path).unwrap();
            let tree = parser.parse(file_content.clone(), None).unwrap();
            (
                path.to_str().unwrap().to_string(),
                ParseResult {
                    parser,
                    tree,
                    file_content,
                },
            )
        })
        .collect();

    Ok(parsers)
}

/// Returns true if the given `DirEntry` has an extension equal to one of
/// the given extensions, and false otherwise.
fn check_extensions(dir_entry: &DirEntry, extensions: Vec<String>) -> bool {
    extensions.iter().any(|ex| has_extension(dir_entry, ex))
}

/// Returns true if the given `DirEntry`'s extension is equal to the given
/// extension.
fn has_extension(dir_entry: &DirEntry, target_ext: &str) -> bool {
    dir_entry
        .path()
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == target_ext)
        .unwrap_or(false)
}

/// Returns all the capture names (names starting with '@') in the given query source in
/// the same order they appear.
///
/// This is different from `Query::capture_names` which returns a list of
/// unique capture names.
fn get_capture_names(query: &Query, query_src: String) -> Vec<String> {
    let start_bytes: Vec<usize> = (0..query.pattern_count())
        .map(|i| query.start_byte_for_pattern(i))
        .collect();

    let mut patterns = vec![];
    for pat_idx in 1..=start_bytes.len() {
        let mut query_src = query_src.clone();
        let start_byte = start_bytes[pat_idx - 1];
        let mut drained: String = if pat_idx != start_bytes.len() {
            query_src.drain(start_byte..start_bytes[pat_idx]).collect()
        } else {
            query_src.drain(start_byte..).collect()
        };
        let query_start = drained.find('@').unwrap() + 1;
        let mut drained: String = drained.drain(query_start..).collect();
        let query_end = drained.find(|c| c == '\n' || c == ' ' || c == ')').unwrap();
        let query_name: String = drained.drain(..query_end).collect();
        patterns.push(query_name);
    }

    patterns
}
