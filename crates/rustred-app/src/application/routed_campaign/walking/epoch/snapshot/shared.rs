//! Process-local immutable-root storage for lookup snapshots.
//!
//! A root clone shares everything. A writer copies only shared nodes on the
//! path to a changed page; untouched pages and branches keep their identity.
//! Neither the canonical domain arena nor a checkpoint encoding uses this
//! representation. Fallible vector preparation happens before changing an
//! element or the visible length. `Arc` allocations have Rust's ordinary
//! allocation-failure behavior; this does not promise recovery from global OOM.
use std::sync::Arc;

const PAGE_BITS: u32 = 6;
const PAGE: usize = 1 << PAGE_BITS;
const BRANCH_BITS: u32 = 5;
const BRANCH: usize = 1 << BRANCH_BITS;

enum Node<T: Copy> {
    Page(Vec<T>),
    Branch(Vec<Arc<Node<T>>>),
}

/// Diagnostic allocation work, not RSS or a claim about allocator overhead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Copies {
    pub nodes: usize,
    pub values: usize,
    pub pointers: usize,
}

impl<T: Copy> Node<T> {
    fn copy(
        &self,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Self, &'static str> {
        checkpoint()?;
        let node = match self {
            Self::Page(values) => {
                let mut copy = Vec::new();
                copy.try_reserve_exact(PAGE)
                    .map_err(|_| "snapshot shared page allocation")?;
                copy.extend_from_slice(values);
                work.values += values.len();
                Self::Page(copy)
            }
            Self::Branch(children) => {
                let mut copy = Vec::new();
                copy.try_reserve_exact(BRANCH)
                    .map_err(|_| "snapshot shared branch allocation")?;
                copy.extend(children.iter().cloned());
                work.pointers += children.len();
                Self::Branch(copy)
            }
        };
        work.nodes += 1;
        Ok(node)
    }
}

/// Dense u32-addressed pages. Its root and each populated path have logarithmic
/// metadata; appending does not copy an ever-growing vector of page pointers.
pub(in super::super) struct Pages<T: Copy> {
    root: Option<Arc<Node<T>>>,
    len: usize,
    height: u32,
}

impl<T: Copy> Clone for Pages<T> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            len: self.len,
            height: self.height,
        }
    }
}

impl<T: Copy> Default for Pages<T> {
    fn default() -> Self {
        Self {
            root: None,
            len: 0,
            height: 0,
        }
    }
}

impl<T: Copy> std::ops::Index<usize> for Pages<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        self.get(index).expect("shared snapshot ID in range")
    }
}

#[cfg(test)]
impl<T: Copy + std::fmt::Debug> std::fmt::Debug for Pages<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_list()
            .entries((0..self.len()).map(|i| &self[i]))
            .finish()
    }
}

#[cfg(test)]
impl<T: Copy + PartialEq> PartialEq<Vec<T>> for Pages<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        self.len() == other.len()
            && other
                .iter()
                .enumerate()
                .all(|(i, v)| self.get(i) == Some(v))
    }
}

#[cfg(test)]
impl<T: Copy + PartialEq, const M: usize> PartialEq<[T; M]> for Pages<T> {
    fn eq(&self, other: &[T; M]) -> bool {
        self.len() == M
            && other
                .iter()
                .enumerate()
                .all(|(i, v)| self.get(i) == Some(v))
    }
}

impl<T: Copy> Pages<T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let mut node = self.root.as_deref()?;
        let mut height = self.height;
        loop {
            match node {
                Node::Page(values) => return values.get(index & (PAGE - 1)),
                Node::Branch(children) => {
                    let shift = PAGE_BITS + BRANCH_BITS * (height - 1);
                    node = children.get((index >> shift) & (BRANCH - 1))?;
                    height -= 1;
                }
            }
        }
    }

    /// Visit complete and partial pages in logical order without per-element
    /// tree walks or an allocated iterator stack. The callback may stop early.
    pub fn for_each_page(
        &self,
        mut visit: impl FnMut(&[T]) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        fn walk<T: Copy>(
            node: &Node<T>,
            visit: &mut impl FnMut(&[T]) -> Result<(), &'static str>,
        ) -> Result<(), &'static str> {
            match node {
                Node::Page(values) => visit(values),
                Node::Branch(children) => {
                    for child in children {
                        walk(child, visit)?;
                    }
                    Ok(())
                }
            }
        }
        if let Some(root) = &self.root {
            walk(root, &mut visit)?;
        }
        Ok(())
    }

    pub(super) fn try_push(
        &mut self,
        value: T,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        // The largest valid domain ID is u32::MAX - 1; len itself can equal
        // u32::MAX. Keep all tree shifts bounded independently of usize width.
        if self.len >= u32::MAX as usize {
            return Err("snapshot shared page ID range");
        }
        if self.root.is_none() {
            self.root = Some(Self::singleton(0, value, work, checkpoint)?);
        } else if self.len as u64 == (PAGE as u64) << (BRANCH_BITS * self.height) {
            // Build the new sibling and parent off to the side. A failure
            // cannot turn a full old root into a visible incomplete branch.
            let sibling = Self::singleton(self.height, value, work, checkpoint)?;
            checkpoint()?;
            let mut children = Vec::new();
            children
                .try_reserve_exact(BRANCH)
                .map_err(|_| "snapshot shared branch allocation")?;
            children.push(Arc::clone(self.root.as_ref().expect("populated root")));
            children.push(sibling);
            work.nodes += 1;
            work.pointers += 1;
            self.root = Some(Arc::new(Node::Branch(children)));
            self.height += 1;
        } else {
            Self::write(
                self.root.as_mut().expect("populated root"),
                self.height,
                self.len,
                value,
                true,
                work,
                checkpoint,
            )?;
        }
        self.len += 1;
        Ok(())
    }

    pub(super) fn try_set(
        &mut self,
        index: usize,
        value: T,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if index >= self.len {
            return Err("snapshot shared page update range");
        }
        Self::write(
            self.root.as_mut().expect("populated root"),
            self.height,
            index,
            value,
            false,
            work,
            checkpoint,
        )
    }

    fn singleton(
        height: u32,
        value: T,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Arc<Node<T>>, &'static str> {
        checkpoint()?;
        let node = if height == 0 {
            let mut values = Vec::new();
            values
                .try_reserve_exact(PAGE)
                .map_err(|_| "snapshot shared page allocation")?;
            values.push(value);
            Node::Page(values)
        } else {
            let child = Self::singleton(height - 1, value, work, checkpoint)?;
            let mut children = Vec::new();
            children
                .try_reserve_exact(BRANCH)
                .map_err(|_| "snapshot shared branch allocation")?;
            children.push(child);
            Node::Branch(children)
        };
        work.nodes += 1;
        Ok(Arc::new(node))
    }

    fn write(
        root: &mut Arc<Node<T>>,
        height: u32,
        index: usize,
        value: T,
        append: bool,
        work: &mut Copies,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if Arc::get_mut(root).is_none() {
            *root = Arc::new(root.copy(work, checkpoint)?);
        }
        match Arc::get_mut(root).expect("private changed path") {
            Node::Page(values) => {
                if append {
                    debug_assert_eq!(index & (PAGE - 1), values.len());
                    values.push(value); // capacity was reserved to PAGE
                } else {
                    values[index & (PAGE - 1)] = value;
                }
            }
            Node::Branch(children) => {
                let shift = PAGE_BITS + BRANCH_BITS * (height - 1);
                let slot = (index >> shift) & (BRANCH - 1);
                if slot == children.len() && append {
                    let child = Self::singleton(height - 1, value, work, checkpoint)?;
                    children.push(child); // capacity was reserved to BRANCH
                } else {
                    Self::write(
                        &mut children[slot],
                        height - 1,
                        index,
                        value,
                        append,
                        work,
                        checkpoint,
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
