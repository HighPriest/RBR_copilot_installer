/// Nodes in a flat arena, so a node's index can ride in a tree view item's `lParam`.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub(super) nodes: Vec<super::Node>,
    /// The top-level category rows, in display order.
    pub(super) roots: Vec<usize>,
}

impl Tree {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(&self, index: usize) -> &super::Node {
        &self.nodes[index]
    }

    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    pub fn children(&self, index: usize) -> &[usize] {
        &self.nodes[index].children
    }

    /// Every leaf under `index`, or in the whole tree for `None`, in display order.
    pub fn leaves_under(&self, index: Option<usize>) -> Vec<usize> {
        let mut leaves = Vec::new();
        match index {
            Some(index) => self.collect_leaves(index, &mut leaves),
            None => {
                for &root in &self.roots {
                    self.collect_leaves(root, &mut leaves);
                }
            }
        }
        leaves
    }

    fn collect_leaves(&self, index: usize, leaves: &mut Vec<usize>) {
        if self.nodes[index].is_leaf() {
            leaves.push(index);
            return;
        }
        for &child in &self.nodes[index].children {
            self.collect_leaves(child, leaves);
        }
    }

    pub(super) fn push(&mut self, node: super::Node, parent: Option<usize>) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        match parent {
            Some(parent) => self.nodes[parent].children.push(index),
            None => self.roots.push(index),
        }
        index
    }
}
