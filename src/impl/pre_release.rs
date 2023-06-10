use crate::r#impl::storage::PreReleasePatternEntry;

pub fn parse_pre_release<'a>(
    version: &str,
    pre_release_patterns: &'a [PreReleasePatternEntry],
) -> Option<&'a str> {
    for pattern in pre_release_patterns {
        if pattern.pattern.is_match(version) {
            return Some(&pattern.display_name);
        }
    }
    None
}
