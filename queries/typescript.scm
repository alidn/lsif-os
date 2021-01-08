; Scopes

(program) @scope

[
    (statement_block)
] @scope

; Definitions

(export_statement
    value: (_
             name: [(type_identifier) (identifier)] @definition.exported))

(export_statement
    declaration: (_
                    name: [(type_identifier) (identifier)] @definition.exported))

(export_statement
    declaration: (_ 
                    (variable_declarator
                        name: (identifier) @definition.exported)))

(variable_declarator
    name: (identifier) @definition.scoped)

(class_declaration
    name: (type_identifier) @definition.scoped)


(method_definition
    name: (property_identifier) @definition.scoped)

(variable_declarator
    name: (_
            (identifier) @definition.scoped))

(formal_parameters
    (_ 
        [
            (identifier) @definition.scoped
        ])
    )
    
(public_field_definition
    name: (property_identifier) @definition.scoped)

(property_signature
	name: (_) @definition.scoped)

(type_alias_declaration
    name: (_) @definition.scoped)

(function_declaration
    name: (identifier) @definition.scoped)

; References

(identifier) @reference

(property_identifier) @reference

(type_identifier) @reference

; Comment

(comment) @comment
