use std::fs;

use crate::protocol::types::Language;

pub trait LanguageTools {
    fn get_extensions(&self) -> Vec<String>;
    fn get_queries(&self) -> String;

    fn get_files(&self, project_root: &str) -> Vec<String> {
        let mut entries = vec![];
        let extensions = self.get_extensions();
        iterate_entries(&mut entries, &extensions, project_root.to_string());
        entries
    }
}

fn iterate_entries(entries: &mut Vec<String>, extensions: &Vec<String>, cwd: String) {
    for entry in fs::read_dir(cwd).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let extension = path.extension();
        let filepath = path.to_str().unwrap().to_owned();
        let filetype = entry.file_type().unwrap();

        if filetype.is_dir() {
            iterate_entries(entries, extensions, filepath);
        } else if filetype.is_file() && extension.is_some() {
            let extension = extension.unwrap().to_str().unwrap();
            let is_project_file = extensions.iter().any(|ext| extension == *ext);
            if is_project_file {
                entries.push(filepath);
            }
        }
    }
}

impl LanguageTools for Language {
    fn get_extensions(&self) -> Vec<String> {
        match self {
            Language::JavaScript => vec!["js".to_string()],
            Language::GraphQL => vec!["graphql".to_string()],
            Language::Lua => vec!["lua".to_string()],
            Language::Java => vec!["java".to_string()],
            Language::TypeScript => vec!["ts".to_string()],
        }
    }

    fn get_queries(&self) -> String {
        match self {
            Language::JavaScript => include_str!("../../queries/javascript.scm"),
            Language::GraphQL => include_str!("../../queries/graphql.scm"),
            Language::Lua => include_str!("../../queries/lua.scm"),
            Language::Java => include_str!("../../queries/java.scm"),
            Language::TypeScript => include_str!("../../queries/typescript.scm"),
        }
        .to_string()
    }
}
