#[path = "../build_revision.rs"]
mod build_revision;

use std::path::Path;

#[test]
fn explicit_full_sha_is_shortened() {
    let short = build_revision::select_short_sha(
        Some("0123456789abcdef0123456789abcdef01234567"),
        Path::new("/path/that/does/not/exist"),
    )
    .unwrap();

    assert_eq!(short.as_deref(), Some("0123456"));
}

#[test]
fn explicit_sha_takes_precedence_over_git_checkout() {
    let short = build_revision::select_short_sha(
        Some("abcdef0123456789abcdef0123456789abcdef01"),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();

    assert_eq!(short.as_deref(), Some("abcdef0"));
}

#[test]
fn malformed_explicit_sha_fails_closed() {
    for value in [
        "0123456",
        "0123456789abcdef0123456789abcdef0123456g",
        "ABCDEF0123456789abcdef0123456789abcdef01",
    ] {
        let error = build_revision::select_short_sha(Some(value), Path::new("."))
            .expect_err("malformed explicit provenance must fail");
        assert!(error.contains(build_revision::BUILD_GIT_SHA_ENV));
    }
}

#[test]
fn missing_explicit_sha_without_git_metadata_is_unstamped() {
    let short =
        build_revision::select_short_sha(None, Path::new("/path/that/does/not/exist")).unwrap();

    assert_eq!(short, None);
}
