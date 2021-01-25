use std::str::FromStr;

use languageserver_types as lsp;
pub use languageserver_types::{NumberOrString, Range};
pub use lsp::*;
use serde::{Deserialize, Serialize};

pub type ID = u64;
pub type RangeId = lsp::NumberOrString;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: lsp::NumberOrString,
    #[serde(flatten)]
    pub data: Element,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Element {
    Vertex(Vertex),
    Edge(Edge),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "label")]
pub enum Vertex {
    Project(Project),
    Document(Document),
    Range(Range),
    ResultSet(ResultSet),
    HoverResult(HoverResult),
    MetaData(MetaData),
    Moniker(Moniker),

    // Method results
    DefinitionResult(DefinitionResult),

    ReferenceResult(ReferenceResult),
    DiagnosticResult,
    ExportResult,
    ExternalImportResult,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "label")]
pub enum Edge {
    Contains(MultiEdgeData),
    RefersTo(EdgeData),
    Next(EdgeData),
    Moniker(EdgeData),

    Item(Item),

    // Methods
    #[serde(rename = "textDocument/definition")]
    Definition(EdgeData),
    #[serde(rename = "textDocument/declaration")]
    Declaration(EdgeData),
    #[serde(rename = "textDocument/hover")]
    Hover(EdgeData),
    #[serde(rename = "textDocument/references")]
    References(EdgeData),
    #[serde(rename = "textDocument/implementation")]
    Implementation(EdgeData),
    #[serde(rename = "textDocument/typeDefinition")]
    TypeDefinition(EdgeData),
    #[serde(rename = "textDocument/foldingRange")]
    FoldingRange(EdgeData),
    #[serde(rename = "textDocument/documentLink")]
    DocumentLink(EdgeData),
    #[serde(rename = "textDocument/documentSymbol")]
    DocumentSymbol(EdgeData),
    #[serde(rename = "textDocument/diagnostic")]
    Diagnostic(EdgeData),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EdgeData {
    pub in_v: lsp::NumberOrString,
    pub out_v: lsp::NumberOrString,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MultiEdgeDataWithDocument {
    pub document: u64,
    pub in_vs: Vec<lsp::NumberOrString>,
    pub out_v: lsp::NumberOrString,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MultiEdgeData {
    pub in_vs: Vec<lsp::NumberOrString>,
    pub out_v: lsp::NumberOrString,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DefinitionResultType {
    Scalar(LocationOrRangeId),
    Array(LocationOrRangeId),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "property")]
pub enum Item {
    Definition(MultiEdgeDataWithDocument),
    Reference(MultiEdgeDataWithDocument),
    #[serde(rename = "")]
    Neither(MultiEdgeDataWithDocument),
}

impl Edge {
    pub fn item(out_v: ID, in_vs: Vec<ID>, doc_id: ID) -> Self {
        Self::Item(Item::Neither(MultiEdgeDataWithDocument {
            document: doc_id,
            in_vs: in_vs.iter().map(|v| NumberOrString::Number(*v)).collect(),
            out_v: NumberOrString::Number(out_v),
        }))
    }

    pub fn def_item(out_v: ID, in_vs: Vec<ID>, doc_id: ID) -> Self {
        Self::Item(Item::Definition(MultiEdgeDataWithDocument {
            document: doc_id,
            in_vs: in_vs.iter().map(|v| NumberOrString::Number(*v)).collect(),
            out_v: NumberOrString::Number(out_v),
        }))
    }

    pub fn ref_item(out_v: ID, in_vs: Vec<ID>, doc_id: ID) -> Self {
        Self::Item(Item::Reference(MultiEdgeDataWithDocument {
            document: doc_id,
            in_vs: in_vs.iter().map(|v| NumberOrString::Number(*v)).collect(),
            out_v: NumberOrString::Number(out_v),
        }))
    }

    pub fn contains(out_v: ID, in_vs: Vec<ID>) -> Self {
        Self::Contains(MultiEdgeData {
            in_vs: in_vs.iter().map(|v| NumberOrString::Number(*v)).collect(),
            out_v: NumberOrString::Number(out_v),
        })
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    #[serde(with = "url_serde")]
    pub uri: lsp::Url,
    pub language_id: Language,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResultSet {}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HoverResult {
    pub(crate) result: Contents,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub contents: Vec<LSIFMarkedString>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LSIFMarkedString {
    pub language: String,
    pub value: String,
    pub is_raw_string: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionResult {}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceResult {}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetaData {
    pub(crate) version: String,
    pub(crate) position_encoding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_info: Option<ToolInfo>,
    #[serde(with = "url_serde")]
    pub(crate) project_root: lsp::Url,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Moniker {
    pub(crate) kind: String,
    pub(crate) scheme: String,
    pub(crate) identifier: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) args: Option<Vec<String>>,
}

impl Default for ToolInfo {
    fn default() -> Self {
        ToolInfo {
            name: "Zas-LSIF-Generator".to_string(),
            version: None,
            args: None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub language_id: Language,
}

/// This enum represents all the currently supported languages.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    JavaScript,
    GraphQL,
    Lua,
    Java,
    TypeScript,
}

impl Language {
    pub fn get_extensions(&self) -> Vec<String> {
        match self {
            Language::JavaScript => vec!["js".to_string()],
            Language::GraphQL => vec!["graphql".to_string()],
            Language::Lua => vec!["lua".to_string()],
            Language::Java => vec!["java".to_string()],
            Language::TypeScript => vec!["ts".to_string(), "tsx".to_string()],
        }
    }

    /// Returns the content of the corresponding query file.
    pub fn get_query_source(&self) -> String {
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

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Language::*;

        match &s.to_lowercase()[..] {
            "javascript" => Ok(JavaScript),
            "graphql" => Ok(GraphQL),
            "lua" => Ok(Lua),
            "java" => Ok(Java),
            "typescript" => Ok(TypeScript),
            _ => Err("Language not supported".to_string()),
        }
    }
}

impl ToString for Language {
    fn to_string(&self) -> String {
        match self {
            Language::JavaScript => "JavaScript",
            Language::GraphQL => "GraphQL",
            Language::Lua => "Lua",
            Language::Java => "Java",
            Language::TypeScript => "TypeScript",
        }
        .to_string()
    }
}

/// # Examples
/// The following code defines a edge of type `Next` going from `a` to `b`
///
/// ```
/// let a = 3;
/// let b = 3;
/// let edge = edge!(Next, a -> b);
/// ```
#[macro_export]
macro_rules! edge {
    ($kind: ident, $from: ident -> $to: ident) => {
        Edge::$kind(EdgeData {
            in_v: NumberOrString::Number($to),
            out_v: NumberOrString::Number($from),
        })
    };

    ($kind: ident, $from: ident -> $to: ident.$field: ident) => {
        Edge::$kind(EdgeData {
            in_v: NumberOrString::Number($to.$field),
            out_v: NumberOrString::Number($from),
        })
    };

    ($kind: ident, $from: ident.$field: ident -> $to: ident) => {
        Edge::$kind(EdgeData {
            in_v: NumberOrString::Number($to),
            out_v: NumberOrString::Number($from.$field),
        })
    };
}

/// Implements the `From` trait for the given enum and enum variant.
/// The first parameter is the enum variant and the second is the enum.
///
/// # Examples
///
/// ```
/// enum StringOrVec {
///     String(String),
///     Vec(Vec<String>)
/// }
///
/// impl_from_variant(String, StringOrVec);
/// impl_from_variant(Vec<String>, StringOrVec);
///
/// fn ex(s: String) -> StringOrVec {
///     s.into()
/// }
/// ```
#[macro_export]
macro_rules! impl_from_variant {
    ($variant: ident, $en: ident) => {
        impl From<$variant> for $en {
            fn from(v: $variant) -> $en {
                $en::$variant(v)
            }
        }
    };
}

impl_from_variant!(Project, Vertex);
impl_from_variant!(Document, Vertex);
impl_from_variant!(Range, Vertex);
impl_from_variant!(ResultSet, Vertex);
impl_from_variant!(MetaData, Vertex);
impl_from_variant!(ReferenceResult, Vertex);
impl_from_variant!(DefinitionResult, Vertex);
impl_from_variant!(HoverResult, Vertex);
impl_from_variant!(Moniker, Vertex);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LocationOrRangeId {
    Location(lsp::Location),
    RangeId(RangeId),
}
