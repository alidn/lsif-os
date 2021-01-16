# LSIF-OS

This is a (mostly) language-agnostic command-line tool for generating [LSIF](https://lsif.dev/) data.

Currently supported languages:
- Graphql
- TypeScript
- JavaScript
- Java

Currently, only TypeScript support is precise enough.

## Installation

Binary download for MacOS is available on the [release tab](https://github.com/alidn/lsif-os/releases).

_Note: If you want to run it locally, you need to run `clone_parsers` to compile the program._

## Performance

fast & parallelized:
| Repository | Index time | Lines | Code | Language |
| ------------------------ | ---------- | :----- | :----- | ---------- |
| rome/**Tools** | ~2.1s | 277317 | 185412 | TypeScript |
| facebook/**Jest** | ~480ms | 85563 | 67512 | TypeScript |
| reduxjs/**Redux** | ~50ms | 4018 | 3028 | TypeScript |

Benchmarks were run on a 4-core MacBook Pro.

## How it works:

1. It uses [treesitter](https://github.com/tree-sitter/tree-sitter) to parse files and produce a treesitter AST.

2. With the language specific query file (in the `queries` directory), it finds the query matches using treesitter. (see `src/analyzer/analyzer.rs`)
   At the end of this step, it has found the following:
   - All definitions:
     - The exact location (file and offset)
     - Whether or not it was exported - Name and documentation
   - All references:
     - It's defintion if it was defined in the same file it was used
     - Name and exact location (file and offset)

For incremental indexing: At this step, we can keep a cache of refernces and definitions (even unlinked onces). After a commit, we only need to reparse the files
that were modified, and update the cache accordingly, then move on to the next step to produce the graph. This can decrease the indexing time by half.

3. Once all files are analyzed, we can find the definition locations for each reference whose definition is in a different file. (see `src/indexer/indexer.rs`)

4. It produces the LSIF graph. (see `src/indexer/indexer.rs`)

--

For a good example of what a query file looks like, see `queries/typescript.scm`; it is more complete that others.
