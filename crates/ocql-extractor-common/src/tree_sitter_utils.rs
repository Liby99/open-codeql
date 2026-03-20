use tree_sitter::Node;

/// Extension trait for tree-sitter nodes with convenience methods.
pub trait NodeExt {
    /// Get a named child by field name, returning None if absent.
    fn child_by_field(&self, name: &str) -> Option<Node<'_>>;

    /// Get the text content of this node from the source.
    fn text<'a>(&self, source: &'a [u8]) -> &'a str;

    /// Iterate over named children (skipping anonymous punctuation nodes).
    fn named_children_iter(&self) -> NamedChildIter<'_>;
}

impl NodeExt for Node<'_> {
    fn child_by_field(&self, name: &str) -> Option<Node<'_>> {
        self.child_by_field_name(name)
    }

    fn text<'a>(&self, source: &'a [u8]) -> &'a str {
        self.utf8_text(source).unwrap_or("")
    }

    fn named_children_iter(&self) -> NamedChildIter<'_> {
        NamedChildIter {
            cursor: self.walk(),
            first: true,
        }
    }
}

/// Iterator over named children of a tree-sitter node.
pub struct NamedChildIter<'a> {
    cursor: tree_sitter::TreeCursor<'a>,
    first: bool,
}

impl<'a> Iterator for NamedChildIter<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.first {
            self.first = false;
            if !self.cursor.goto_first_child() {
                return None;
            }
        } else if !self.cursor.goto_next_sibling() {
            return None;
        }

        // Skip to next named node
        loop {
            let node = self.cursor.node();
            if node.is_named() {
                return Some(node);
            }
            if !self.cursor.goto_next_sibling() {
                return None;
            }
        }
    }
}

/// Walk a tree-sitter tree depth-first, calling `visitor` on each named node.
/// The visitor receives the node, source bytes, and a depth counter.
pub fn walk_tree<F>(node: &Node<'_>, source: &[u8], mut visitor: F)
where
    F: FnMut(&Node<'_>, &[u8], usize),
{
    walk_tree_inner(node, source, 0, &mut visitor);
}

fn walk_tree_inner<F>(node: &Node<'_>, source: &[u8], depth: usize, visitor: &mut F)
where
    F: FnMut(&Node<'_>, &[u8], usize),
{
    if node.is_named() {
        visitor(node, source, depth);
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            walk_tree_inner(&child, source, depth + 1, visitor);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
