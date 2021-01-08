use crate::{analyzer::ffi::query_for_language, protocol::types::Language};

/// Tests whether the query files are valid
#[test]
fn test_query_files() {
    for lang in [
        Language::GraphQL,
        Language::Java,
        Language::JavaScript,
        Language::TypeScript,
    ]
    .iter()
    {
        query_for_language(lang).unwrap();
    }
}
