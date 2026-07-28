//! Release versioning rules (g02.028 batch 28.1).
//!
//! A release is `<bundle> as v1.2.0` — semver is the identity, and
//! channels are retired. Gates already do staged promotion, prerelease
//! tags do pre-release tracks, and `latest` is a computation rather
//! than a pointer anyone can move.
//!
//! Pure functions over parsed versions, for the same reason the gate
//! rules are: this is the part most likely to be wrong in an
//! interesting way, and prerelease ordering is exactly the kind of
//! thing not to hand-roll — hence the `semver` crate underneath.

use semver::Version;

/// Parse a version as a release identity.
///
/// Accepts an optional leading `v`, because that is how people write
/// versions everywhere else and refusing `v1.2.0` would be pedantry
/// with an error message. Stored and displayed without it.
pub fn parse_version(given: &str) -> Result<Version, String> {
    let bare = given.strip_prefix('v').unwrap_or(given);
    Version::parse(bare).map_err(|err| {
        format!("{given} is not a semver version ({err}); expected the shape 1.2.3, 1.2.3-beta.1")
    })
}

/// Why a proposed release version is refused.
///
/// Only uniqueness, by decision (operator, 2026-07-28): versions do not
/// have to increase, because cutting `1.1.1` while `2.0.0` exists is
/// how long-term support works. Strictly-increasing is a later opt-in
/// policy, not the default.
pub fn refuse_version(proposed: &Version, existing: &[Version]) -> Option<String> {
    if existing.contains(proposed) {
        return Some(format!(
            "version {proposed} already exists; a release is immutable, so pick the next number \
             (fix forward, never re-tag)"
        ));
    }
    None
}

/// The version `--release latest` resolves to: highest non-yanked,
/// non-prerelease version. `None` when nothing qualifies — a repo whose
/// only releases are prereleases has no `latest`, on purpose, because
/// handing a beta to somebody who asked for latest is worse than
/// making them name it.
pub fn latest<'a, I>(releases: I) -> Option<&'a Version>
where
    I: IntoIterator<Item = (&'a Version, bool)>,
{
    releases
        .into_iter()
        .filter(|(version, yanked)| !yanked && version.pre.is_empty())
        .map(|(version, _)| version)
        .max()
}

/// Resolve a requested version or range against what exists.
///
/// Three forms, in order of specificity:
/// - `latest` — the computation above
/// - an exact version — found even if yanked, because somebody naming a
///   version exactly is allowed to reach a withdrawn one on purpose
///   (reproducing a bug report against it, say)
/// - a range (`1.x`, `1.2`, `>=1, <2`) — highest non-yanked,
///   non-prerelease match; yanked releases leave ranges just as they
///   leave `latest`
pub fn resolve<'a>(
    request: &str,
    releases: &'a [(Version, bool)],
) -> Result<&'a Version, String> {
    if request == "latest" {
        return latest(releases.iter().map(|(v, y)| (v, *y)))
            .ok_or_else(|| "no releases yet (prereleases and yanked ones do not count)".into());
    }
    if let Ok(exact) = parse_version(request) {
        return releases
            .iter()
            .map(|(v, _)| v)
            .find(|v| **v == exact)
            .ok_or_else(|| format!("no release {exact}"));
    }
    let range = semver::VersionReq::parse(request)
        .map_err(|err| format!("{request} is neither a version nor a range ({err})"))?;
    releases
        .iter()
        .filter(|(v, yanked)| !yanked && v.pre.is_empty() && range.matches(v))
        .map(|(v, _)| v)
        .max()
        .ok_or_else(|| format!("nothing matches {request}"))
}

/// The version assigned to a release that predates versioning: `0.<n>.0`
/// by release order. Deterministic, so every replica of a deployment
/// numbers its history identically — and real numbers rather than a
/// "legacy" label, because a permanent unversioned caste would
/// contradict the rule this feature exists to state.
pub fn migration_version(order: u64) -> Version {
    Version::new(0, order, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        parse_version(s).unwrap()
    }

    #[test]
    fn a_leading_v_is_accepted_and_not_stored() {
        assert_eq!(v("v1.2.3"), v("1.2.3"));
        assert_eq!(v("1.2.3").to_string(), "1.2.3");
    }

    #[test]
    fn nonsense_is_refused_with_the_expected_shape() {
        let err = parse_version("stable").unwrap_err();
        assert!(err.contains("1.2.3"), "{err}");
    }

    #[test]
    fn duplicates_are_refused_and_backports_are_not() {
        let existing = vec![v("1.0.0"), v("2.0.0")];
        assert!(refuse_version(&v("2.0.0"), &existing).is_some());
        // The deal-breaker case: an LTS fix below the newest version.
        assert!(
            refuse_version(&v("1.0.1"), &existing).is_none(),
            "backports must work by default"
        );
    }

    #[test]
    fn latest_skips_prereleases_and_yanks() {
        let releases = vec![
            (v("1.0.0"), false),
            (v("2.0.0"), true),          // yanked
            (v("2.1.0-beta.1"), false),  // prerelease
            (v("1.5.0"), false),
        ];
        assert_eq!(
            latest(releases.iter().map(|(v, y)| (v, *y))),
            Some(&v("1.5.0"))
        );
    }

    #[test]
    fn a_backport_does_not_change_latest() {
        let mut releases = vec![(v("2.0.0"), false)];
        releases.push((v("1.0.1"), false));
        assert_eq!(
            latest(releases.iter().map(|(v, y)| (v, *y))),
            Some(&v("2.0.0"))
        );
    }

    #[test]
    fn resolve_handles_latest_exact_and_ranges() {
        let releases = vec![
            (v("1.0.0"), false),
            (v("1.2.0"), false),
            (v("2.0.0"), false),
            (v("2.1.0"), true), // yanked
        ];
        assert_eq!(resolve("latest", &releases).unwrap(), &v("2.0.0"));
        assert_eq!(resolve("1.x", &releases).unwrap(), &v("1.2.0"));
        // Naming a yanked version exactly still reaches it.
        assert_eq!(resolve("2.1.0", &releases).unwrap(), &v("2.1.0"));
        // But a range never resolves to it.
        assert_eq!(resolve("2.x", &releases).unwrap(), &v("2.0.0"));
        assert!(resolve("3.x", &releases).is_err());
    }

    #[test]
    fn a_prerelease_only_repo_has_no_latest() {
        let releases = vec![(v("1.0.0-rc.1"), false)];
        let err = resolve("latest", &releases).unwrap_err();
        assert!(err.contains("prereleases"), "{err}");
    }

    #[test]
    fn migration_numbers_are_deterministic_and_ordered() {
        assert_eq!(migration_version(1).to_string(), "0.1.0");
        assert_eq!(migration_version(3).to_string(), "0.3.0");
        assert!(migration_version(1) < migration_version(2));
    }
}
