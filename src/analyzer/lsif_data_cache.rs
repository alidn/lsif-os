use std::{collections::HashMap, sync::Arc};

use super::analyzer::{Definition, Location, Reference};
use crate::analyzer::analyzer::DefinitionKind;
use crate::protocol::types::ID;
use smol_str::SmolStr;

#[derive(Default)]
pub struct LsifDataCache {
    /// Filename -> Info
    documents: HashMap<String, DocumentInfo>,
    /// Filename -> Offset -> Range ID
    ranges: HashMap<String, HashMap<usize, ID>>,
    /// Definition Info Cache
    def_infos: HashMap<Location, DefinitionInfo>,
    /// Exported definitions Cache (Name -> Definition)
    exported_defs: HashMap<SmolStr, Arc<Definition>>,
}

/// Methods for caching and retrieving documents
impl LsifDataCache {
    pub fn cache_document(&mut self, filename: String, document_id: ID) {
        self.documents.insert(
            filename.clone(),
            DocumentInfo {
                id: document_id,
                definition_range_ids: Default::default(),
                reference_range_ids: Default::default(),
            },
        );
        self.ranges.insert(filename, Default::default());
    }

    pub fn get_document_id(&self, filename: &str) -> Option<ID> {
        self.documents.get(filename).map(|d| d.id)
    }

    pub fn get_mut_document(&mut self, filename: &str) -> Option<&mut DocumentInfo> {
        self.documents.get_mut(filename)
    }

    pub fn get_documents(&self) -> impl Iterator<Item = &DocumentInfo> {
        self.documents.iter().map(|(_p, d)| d)
    }

    pub fn get_range_id(&self, filename: &str, offset: usize) -> Option<ID> {
        self.ranges.get(filename).unwrap().get(&offset).map(|v| *v)
    }

    pub fn get_document(&self, filename: &str) -> Option<&DocumentInfo> {
        self.documents.get(filename)
    }
}

/// Methods for retrieving and caching definitions
impl LsifDataCache {
    pub fn get_mut_def_infos(&mut self) -> impl Iterator<Item = &mut DefinitionInfo> {
        self.def_infos.iter_mut().map(|(_loc, def)| def)
    }

    pub fn get_definition_info(&self, location: &Location) -> Option<&DefinitionInfo> {
        self.def_infos.get(location)
    }

    pub fn cache_definition(
        &mut self,
        def: &Arc<Definition>,
        document_id: ID,
        range_id: ID,
        result_set_id: ID,
    ) {
        let file_ranges = self.ranges.get_mut(&def.location.file_path).unwrap();
        file_ranges.insert(def.location.range.start_byte, range_id);

        let document_info = self.get_mut_document(&def.location.file_path).unwrap();
        document_info.definition_range_ids.push(range_id);

        let def_info = DefinitionInfo {
            document_id,
            range_id,
            result_set_id,
            reference_range_ids: Default::default(),
        };
        self.def_infos
            .insert(def.location.clone(), def_info.clone());
        if def.kind == DefinitionKind::Exported {
            self.exported_defs
                .insert(SmolStr::clone(&def.node_name), Arc::clone(def));
        }
    }

    pub fn defs_with_name(&self, name: &SmolStr) -> Option<&Arc<Definition>> {
        self.exported_defs.get(name)
    }
}

/// Methods for caching and retrieving references
impl LsifDataCache {
    pub fn cache_reference(&mut self, def: &Definition, r: &Reference, range_id: ID) {
        {
            let id = self.get_mut_document(&def.location.file_path).unwrap().id;
            let def_info = self.def_infos.get_mut(&def.location).unwrap();
            def_info.reference_range_ids.entry(id).or_default();
            let def_range_ids = def_info.reference_range_ids.get_mut(&id).unwrap();
            def_range_ids.push(range_id);
        }

        let document_info = self.get_mut_document(&r.location.file_path).unwrap();
        document_info.reference_range_ids.push(range_id);
    }

    pub fn cache_reference_range(&mut self, r: &Reference, range_id: ID) {
        let file_ranges = self.ranges.get_mut(&r.location.file_path).unwrap();
        file_ranges.insert(r.location.range.start_byte, range_id);
    }
}

pub struct DocumentInfo {
    pub id: ID,
    pub definition_range_ids: Vec<ID>,
    pub reference_range_ids: Vec<ID>,
}

#[derive(Clone)]
pub struct DefinitionInfo {
    pub document_id: ID,
    pub range_id: ID,
    pub result_set_id: ID,
    /// Document ID -> Range ID
    pub reference_range_ids: HashMap<ID, Vec<ID>>,
}
