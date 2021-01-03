use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc::channel, Arc},
};

use anyhow::Context;
use ignore::{DirEntry, Walk};
use indicatif::{ProgressBar, ProgressStyle};
use languageserver_types::{NumberOrString, Url};
use rayon::prelude::*;
use tree_sitter::{Parser, Query, Tree};

use crate::{
    analyzer::{
        analyzer::{Analyzer, Definition, Reference},
        ffi::{parser_for, query_for_language, ts_language_from},
        lsif_data_cache::{DefinitionInfo, LsifDataCache},
        utils::get_file_content,
    },
    cli::Opts,
    edge,
    emitter::emitter::Emitter,
    protocol::types::{
        Contents, DefinitionResult, Document, Edge, EdgeData, HoverResult, LSIFMarkedString,
        Language, MetaData, ReferenceResult, ResultSet, ToolInfo, ID,
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
    /// Generates an LSIF dump from a workspace by traversing through files of the given language
    /// and emitting the LSIF equivalent using the given emitter.
    pub fn index(opt: Opts, emitter: E) -> anyhow::Result<()> {
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
            let query = query_for_language(&opt.language).unwrap();
            let files = indexer.file_paths();
            let files = get_files_parsers(&opt.language, files)?;
            indexer.emit_definitions(files, &query);
        }
        indexer.link_reference_results_to_ranges();
        indexer.emit_contains();

        indexer.emitter.end();

        Ok(())
    }

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

    fn emit_contains_for_project(&mut self) {
        let document_ids = self.cache.get_documents().map(|d| d.id).collect();
        self.emitter
            .emit_edge(Edge::contains(self.project_id, document_ids));
    }

    fn link_reference_results_to_ranges(&mut self) {
        let def_infos = self.cache.get_mut_def_infos();
        Self::link_items_to_definitions(&def_infos.collect(), &mut self.emitter);
    }

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

    fn emit_definitions(&mut self, files: HashMap<String, (Parser, Tree, String)>, query: &Query) {
        let (def_sender, def_receiver) = channel();
        let (ref_sender, ref_receiver) = channel();

        let capture_names = get_capture_names(&query, self.opt.language.get_queries());

        let bar = ProgressBar::new(files.len() as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{bar:40.cyan/blue} {pos}/{len} files indexed")
                .progress_chars("==>"),
        );
        files.into_par_iter().for_each_with(
            (def_sender, ref_sender),
            |(d, r), (filename, (_, tree, file_content))| {
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

    fn index_reference(&mut self, r: Reference) {
        match &r.def {
            Some(def) => self.index_reference_to_definition(&def, &r),
            None => {
                // TODO: Find the definition which might be a dependency or an import
            }
        }
    }

    fn ensure_range_for(&mut self, r: &Reference) -> ID {
        match self
            .cache
            .get_range_id(&r.location.filename, r.location.range.start_byte)
        {
            Some(range_id) => range_id,
            None => {
                let range_id = self.emitter.emit_vertex(r.range());
                self.cache.cache_reference_range(r, range_id);
                range_id
            }
        }
    }

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

    fn index_definition(&mut self, def: Arc<Definition>) {
        let document_id = self.cache.get_document_id(&def.location.filename).unwrap();

        // 1. Emit Vertices
        let range_id = self.emitter.emit_vertex(def.range());
        let result_set_id = self.emitter.emit_vertex(ResultSet {});
        let def_result_id = self.emitter.emit_vertex(DefinitionResult {});
        let hover_result_id = def.comment.clone().map(|c| {
            self.emitter.emit_vertex(HoverResult {
                result: Contents {
                    contents: vec![LSIFMarkedString {
                        language: self.opt.language.to_string(),
                        value: c,
                        is_raw_string: true,
                    }],
                },
            })
        });

        // 2. Connect the emitted vertices
        self.emitter
            .emit_edge(edge!(Next, range_id -> result_set_id));
        self.emitter
            .emit_edge(edge!(Definition, result_set_id -> def_result_id));
        self.emitter
            .emit_edge(Edge::item(def_result_id, vec![range_id], document_id));
        if let Some(id) = hover_result_id {
            self.emitter.emit_edge(edge!(Hover, result_set_id -> id));
        }

        // 3. Cache the result
        self.cache
            .cache_definition(&def, document_id, range_id, result_set_id);
    }

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

    fn file_paths(&mut self) -> Vec<PathBuf> {
        if let Some(res) = &self.cached_file_paths {
            res.clone()
        } else {
            let exs = self.opt.language.get_extensions();
            let res: Vec<PathBuf> = Walk::new(PathBuf::from(&self.opt.project_root))
                .into_iter()
                .filter(|dir_entry_res| dir_entry_res.is_ok())
                .map(std::result::Result::unwrap)
                .filter(move |entry| {
                    entry.metadata().unwrap().is_file() && check_file(entry, exs.clone())
                })
                .map(DirEntry::into_path)
                .collect();
            self.cached_file_paths = Some(res.clone());
            res
        }
    }
}

fn get_files_parsers(
    lang: &Language,
    files: Vec<PathBuf>,
) -> anyhow::Result<HashMap<String, (Parser, Tree, String)>> {
    let lang = ts_language_from(lang);
    let parsers = files
        .into_par_iter()
        .map(|path| {
            let mut parser = parser_for(lang).unwrap();
            let file_content = get_file_content(&path);
            let tree = parser.parse(file_content.clone(), None).unwrap();
            (
                path.to_str().unwrap().to_string(),
                (parser, tree, file_content),
            )
        })
        .collect();

    Ok(parsers)
}

fn check_file(dir_entry: &DirEntry, extensions: Vec<String>) -> bool {
    for ex in extensions {
        if has_extension(dir_entry, &ex) {
            return true;
        }
    }
    return false;
}

fn has_extension(dir_entry: &DirEntry, target_ext: &str) -> bool {
    dir_entry
        .path()
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == target_ext)
        .unwrap_or(false)
}

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
        println!("==\n{}\n==", drained);
        let query_start = drained.find('@').unwrap() + 1;
        let mut drained: String = drained.drain(query_start..).collect();
        let query_end = drained.find(|c| c == '\n' || c == ' ' || c == ')').unwrap();
        let query_name: String = drained.drain(..query_end).collect();
        patterns.push(query_name);
    }

    patterns
}
