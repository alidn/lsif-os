1. It uses treesitter to parse files and produce a treesitter AST. 

2. With the language specific query file, it finds the query[1] matches using treesitter. At the end of this step, 
it has found the following:
    - All definitions/declarations: 
      - The exact location (file and offset)
      - Whether or not it was exported
      - Name and documentation
    - All references:
      - It's defintion if it was defined in the same file it was used
      - Name and exact location (file and offset)

For incremental indexing: At this step, we can keep a cache of refernces and definitions (even unlinked onces). After a commit, we only need to reparse the files 
that were modified, and update the cache accordingly. This is simple and cheap since it has not linked definitions and references across files.

3. Once all files are analyzed, we can find the definition locations for variables/functions/classes that were defined in a different file.

4. It produces the LSIF graph.


[1] There are currently 4 query types:
  - definition.scoped
  - definition.exported
  - scope
  - comment
  - reference
