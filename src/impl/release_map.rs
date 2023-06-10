use crate::r#impl::pre_release::parse_pre_release;
use crate::r#impl::storage::{PreReleasePatternEntry, VersionInfo};
use indexmap::IndexMap;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
#[cfg(feature = "sort_versions")]
use std::cmp::Ordering;
use std::fmt;
use std::hash::BuildHasher;
use std::marker::PhantomData;
#[cfg(feature = "sort_versions")]
use version_compare::Cmp;

/// A struct that contains a name of a version and whether or not it is the latest version
/// for a product.
pub struct VersionListEntry<'a> {
    pub name: &'a str,
    pub is_latest: bool,
    pub is_pre_release: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
/// A map of versions. The interior index map is always sorted by version number (best-effort, see
/// `version-compare` crate.
pub struct ReleaseMap(
    #[serde(deserialize_with = "ReleaseMap::serde_deserialize")] IndexMap<String, VersionInfo>,
);

impl ReleaseMap {
    /// Returns a map of all versions, sorted by version number.
    #[inline]
    pub fn map(&self) -> &IndexMap<String, VersionInfo> {
        &self.0
    }

    /// Returns an item from the map as a named version.
    #[inline]
    pub fn get<'a>(&'a self, k: &'a str) -> Option<NamedVersion<'a>> {
        self.0.get(k).map(move |v| NamedVersion(k.into(), v.into()))
    }

    // Returns an iterator over the versions, that provides the names of the versions,
    // whether they are the latest version and whether they are a pre-release.
    //
    // The result iterator is sorted from highest version to lowest!
    pub fn list<'a>(
        &'a self,
        pre_release_patterns: &'a [PreReleasePatternEntry],
    ) -> impl IntoIterator<Item = VersionListEntry<'a>> + 'a {
        let mut had_latest = false;
        self.0
            .keys()
            .rev()
            .map(|name| match parse_pre_release(name, pre_release_patterns) {
                None => match had_latest {
                    true => VersionListEntry {
                        name,
                        is_latest: false,
                        is_pre_release: false,
                    },
                    false => {
                        had_latest = true;
                        VersionListEntry {
                            name,
                            is_latest: true,
                            is_pre_release: false,
                        }
                    }
                },
                Some(_) => VersionListEntry {
                    name,
                    is_latest: false,
                    is_pre_release: true,
                },
            })
            .collect::<Vec<_>>()
    }

    /// Returns the latest version or None if the map is empty.
    pub fn latest(&self, pre_release_patterns: &[PreReleasePatternEntry]) -> Option<NamedVersion> {
        self.0
            .iter()
            .rev()
            .find(|(name, _)| parse_pre_release(name, pre_release_patterns).is_none())
            .map(Into::into)
    }

    fn serde_deserialize<'de, D>(deserializer: D) -> Result<IndexMap<String, VersionInfo>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ReleaseVisitor(PhantomData))
    }
}

impl IntoIterator for ReleaseMap {
    type Item = (String, VersionInfo);
    type IntoIter = <IndexMap<String, VersionInfo> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Visitor to deserialize a `ReleaseMap`
struct ReleaseVisitor<S>(PhantomData<S>);

impl<'de, S> Visitor<'de> for ReleaseVisitor<S>
where
    S: Default + BuildHasher,
{
    type Value = IndexMap<String, VersionInfo, S>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let capacity = map.size_hint().unwrap_or(0);

        #[cfg(feature = "sort_versions")]
        {
            // TODO: This is basically what `Itertools::sort_by` does, but ideally
            //       you wouldn't want to allocate a full Vec if possible.
            let mut v: Vec<(String, VersionInfo)> = Vec::with_capacity(capacity);
            while let Some(val) = map.next_entry()? {
                v.push(val);
            }
            v.sort_by(|(k1, _), (k2, _)| {
                version_compare::compare(&k1, &k2)
                    .unwrap_or(Cmp::Lt)
                    .ord()
                    .unwrap_or(Ordering::Less)
            });
            Ok(IndexMap::from_iter(v))
        }
        #[cfg(not(feature = "sort_versions"))]
        {
            let mut imap: IndexMap<String, VersionInfo, S> =
                IndexMap::with_capacity_and_hasher(capacity, S::default());
            while let Some((k, v)) = map.next_entry()? {
                imap.insert(k, v);
            }
            Ok(imap)
        }
    }
}

#[derive(Clone, Debug)]
pub struct NamedVersion<'a>(Cow<'a, str>, Cow<'a, VersionInfo>);

impl<'a> NamedVersion<'a> {
    #[inline]
    pub fn name(&self) -> &str {
        &self.0
    }
    #[inline]
    pub fn info(&self) -> &VersionInfo {
        &self.1
    }
}

impl<'a, N, V> From<(N, V)> for NamedVersion<'a>
where
    N: Into<Cow<'a, str>>,
    V: Into<Cow<'a, VersionInfo>>,
{
    fn from((n, v): (N, V)) -> Self {
        NamedVersion(n.into(), v.into())
    }
}
