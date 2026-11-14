//! [`CowMap`]: the item and job maps of [`super::CampaignState`] with copy-on-write values.
//!
//! A `BTreeMap<String, Arc<V>>` behind the read API of a `BTreeMap<String, V>` (`get`, `[]`, `iter`, `values`, `&map`
//! in `for` loops all yield `&V`; serde form identical to the plain map). Cloning the map copies the keys and bumps
//! one reference count per value, so the ledger can freeze the state at a rotation in milliseconds and serialise
//! the frozen copy on a background thread while appends continue (ledger interface 8, lane ledgerfix2 m10); a value
//! mutated meanwhile is copied once (`Arc::make_mut`). Values live behind a pointer: the B-tree nodes hold 8-byte
//! slots instead of the full records (less memory left unused in partly filled nodes).

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::collections::btree_map;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A sorted string-keyed map whose values are shared copy-on-write (see the module doc).
#[derive(Clone, PartialEq, Eq)]
pub struct CowMap<V> {
    m: BTreeMap<String, Arc<V>>,
}

impl<V> Default for CowMap<V> {
    fn default() -> Self {
        CowMap { m: BTreeMap::new() }
    }
}

/// Iterator of `(&key, &value)` in key order.
pub struct Iter<'a, V>(btree_map::Iter<'a, String, Arc<V>>);

impl<'a, V> Iterator for Iter<'a, V> {
    type Item = (&'a String, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k, &**v))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<V> DoubleEndedIterator for Iter<'_, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(k, v)| (k, &**v))
    }
}

impl<V> ExactSizeIterator for Iter<'_, V> {}

/// Iterator of `&value` in key order.
pub struct Values<'a, V>(btree_map::Values<'a, String, Arc<V>>);

impl<'a, V> Iterator for Values<'a, V> {
    type Item = &'a V;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|v| &**v)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<V> DoubleEndedIterator for Values<'_, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|v| &**v)
    }
}

impl<V> ExactSizeIterator for Values<'_, V> {}

impl<V> CowMap<V> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.m.len()
    }
    pub fn is_empty(&self) -> bool {
        self.m.is_empty()
    }
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.m.get(k).map(|v| &**v)
    }
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.m.contains_key(k)
    }
    pub fn iter(&self) -> Iter<'_, V> {
        Iter(self.m.iter())
    }
    pub fn values(&self) -> Values<'_, V> {
        Values(self.m.values())
    }
    pub fn keys(&self) -> btree_map::Keys<'_, String, Arc<V>> {
        self.m.keys()
    }
    /// True when the value of `k` is shared with a frozen copy (a mutation would copy it).
    pub fn is_shared<Q>(&self, k: &Q) -> bool
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.m.get(k).is_some_and(|v| Arc::strong_count(v) > 1)
    }
}

impl<V: Clone> CowMap<V> {
    /// Mutable access; copies the value first if a frozen copy of the map shares it.
    pub fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.m.get_mut(k).map(Arc::make_mut)
    }
    pub fn insert(&mut self, k: String, v: V) -> Option<V> {
        self.m
            .insert(k, Arc::new(v))
            .map(|old| Arc::try_unwrap(old).unwrap_or_else(|a| (*a).clone()))
    }
    pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.m
            .remove(k)
            .map(|old| Arc::try_unwrap(old).unwrap_or_else(|a| (*a).clone()))
    }
    /// Mutable iteration: copies EVERY shared value (prefer `get_mut` on the keys that change).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut V)> {
        self.m.iter_mut().map(|(k, v)| (k, Arc::make_mut(v)))
    }
}

impl<V, Q> std::ops::Index<&Q> for CowMap<V>
where
    String: Borrow<Q>,
    Q: Ord + ?Sized,
{
    type Output = V;
    fn index(&self, k: &Q) -> &V {
        self.get(k).expect("no entry found for key")
    }
}

impl<'a, V> IntoIterator for &'a CowMap<V> {
    type Item = (&'a String, &'a V);
    type IntoIter = Iter<'a, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<V> FromIterator<(String, V)> for CowMap<V> {
    fn from_iter<I: IntoIterator<Item = (String, V)>>(it: I) -> Self {
        CowMap {
            m: it.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
        }
    }
}

impl<V: std::fmt::Debug> std::fmt::Debug for CowMap<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<V: Serialize> Serialize for CowMap<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.iter())
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for CowMap<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let m: BTreeMap<String, V> = BTreeMap::deserialize(d)?;
        Ok(m.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaves_like_a_btreemap_and_copies_on_write() {
        let mut a: CowMap<Vec<u32>> = CowMap::new();
        a.insert("b".into(), vec![2]);
        a.insert("a".into(), vec![1]);
        assert_eq!(a.len(), 2);
        assert_eq!(a["a"], vec![1]);
        assert_eq!(a.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        let frozen = a.clone();
        assert!(a.is_shared("a"));
        a.get_mut("a").unwrap().push(9);
        assert_eq!(frozen["a"], vec![1], "the frozen copy keeps its value");
        assert_eq!(a["a"], vec![1, 9]);
        assert!(!a.is_shared("a") && a.is_shared("b"));
        let j = serde_json::to_string(&a).unwrap();
        assert_eq!(j, r#"{"a":[1,9],"b":[2]}"#);
        let back: CowMap<Vec<u32>> = serde_json::from_str(&j).unwrap();
        assert_eq!(back, a);
        let plain: BTreeMap<String, Vec<u32>> = serde_json::from_str(&j).unwrap();
        assert_eq!(serde_json::to_string(&plain).unwrap(), j);
    }
}
