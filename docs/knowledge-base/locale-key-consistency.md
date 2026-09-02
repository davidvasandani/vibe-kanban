# Locale key consistency gates

Tags: `vk/94c0-three-loose-ends`

## Compare sets under one collation

`comm` does not merely compare lines; it assumes both inputs were sorted using
the same collation that `comm` itself is using. Setting `LC_ALL=C` only on the
upstream `sort` is insufficient when `comm` inherits another locale such as
`C.UTF-8`. The lists can look sorted to a reader while `comm` emits ordering
warnings and its set difference is no longer trustworthy.

Establish one locale for the entire check before producing or consuming key
lists. Keep `sort -u` at key extraction and retain `set -o pipefail` so malformed
JSON cannot become a successful empty key set.

## A green gate proves more than current completeness

When restoring missing translations, validate all three layers:

1. scalar key sets match the source locale;
2. interpolation identifiers are byte-identical in every translation;
3. plural suffixes match the conventions actually shipped by the locale
   resources.

Running the complete CI-facing command is essential. A direct jq comparison can
prove the data but will not exercise clone/baseline logic, duplicate detection,
or the `sort`/`comm` boundary that caused this incident.
