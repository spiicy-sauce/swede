; Outline panel entries: the recipe/menu, its bases, and its named nodes.

(recipe
  name: (identifier) @name) @item

(menu
  name: (identifier) @name) @item

(basis_decl
  name: (identifier) @name) @item

(binding
  (output_list
    (output name: (identifier) @name))) @item

(menu_stmt
  alias: (identifier) @name) @item
