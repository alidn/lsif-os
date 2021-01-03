(program) @scope


(method_declaration
  name: (identifier) @definition.scoped)

(interface_declaration
  name: (identifier) @definition.scoped)

(class_declaration
  name: (identifier) @definition.scoped)

(enum_declaration
  name: (identifier) @definition.scoped)

(variable_declarator
  name: (identifier) @definition.scoped)

(identifier) @reference

(comment) @comment


[
    (block)
] @scope
