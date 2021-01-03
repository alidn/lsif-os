; Import/Exports

; Scopes

(program) @scope

[
    (statement_block)
] @scope

; Definitions

(function_declaration
    name: (identifier) @definition.scoped)

(export_statement
    declaration: (function_declaration
                    name: (identifier) @definition.exported))

(variable_declarator
    name: (identifier) @definition.scoped)

; References

(identifier) @reference

; Comment

(comment) @comment
