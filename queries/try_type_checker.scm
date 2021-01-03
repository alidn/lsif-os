(var_declarator
    (var_keyword)
    (identifier) @type_target
    (identifier) @type))

(struct_definition
    (field
        (identifier) @type_target
        (identifier) @type
    )
)

(function_declaration
    (identifier) @type_target.func
    ...
    (parameter
        (identifier) @type_target
    )

    (return_type) @type.func
)
