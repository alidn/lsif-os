repositories=(
    "https://github.com/dralletje/tree-sitter-graphql.git"
    "https://github.com/tree-sitter/tree-sitter-javascript.git"
    "https://github.com/tree-sitter/tree-sitter-typescript.git"
    "https://github.com/tree-sitter/tree-sitter-java.git"
)

mkdir -p parsers;
cd parsers || echo "CD parsers failed" exit;

for url in "${repositories[@]}"; do
    git clone "$url"
done