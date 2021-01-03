use anyhow::{anyhow as error, Result};
use tree_sitter::{LanguageError, Parser, Query};

use crate::{protocol::types::Language};

extern "C" {
    fn tree_sitter_javascript() -> tree_sitter::Language;

    fn tree_sitter_graphql() -> tree_sitter::Language;

    fn tree_sitter_java() -> tree_sitter::Language;

    fn tree_sitter_typescript() -> tree_sitter::Language;

    // FIXME: find out why Lua parser doesn't compile
    // fn tree_sitter_lua() -> tree_sitter::Language;
}

pub fn query_for_language(language: &Language) -> Result<Query> {
    let query_src = language.get_queries();
    let query = Query::new(ts_language_from(&language), &query_src).map_err(|e| {
        error!(
            "\n\nError in the query file for the {:?} language: '{}' is not valid {:?}. (line {}, column {})\n",
            language, e.message, e.kind, e.row + 1, e.column + 1,
        )
    })?;
    Ok(query)
}

pub fn parser_for(language: tree_sitter::Language) -> Result<Parser, LanguageError> {
    let mut parser = Parser::new();
    parser.set_language(language)?;
    Ok(parser)
}

pub fn ts_language_from(language: &Language) -> tree_sitter::Language {
    match language {
        Language::JavaScript => unsafe { tree_sitter_javascript() },
        Language::GraphQL => unsafe { tree_sitter_graphql() },
        Language::Java => unsafe { tree_sitter_java() },
        Language::Lua => unsafe { panic!() },
        Language::TypeScript => unsafe { tree_sitter_typescript() },
    }
}
