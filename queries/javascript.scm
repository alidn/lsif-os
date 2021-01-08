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

(formal_parameters
    [
        (identifier) @definition.scoped
    ])

(variable_declarator
    name: (identifier) @definition.scoped)

(export_statement
    declaration: (class_declaration
                    name: (identifier) @definition.exported))

(class_declaration
    name: (identifier) @definition.scoped)


; References

(identifier) @reference

; Comment

(comment) @comment
