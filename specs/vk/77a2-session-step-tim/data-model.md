# Data model and contract

No wire/schema changes. Existing nullable NormalizedEntry.timestamp carries a
vendor time when present, otherwise the first live server observation time.
Observation time is retained by `/entries/<numeric index>` while its identity
exists. A valid replacement source time overrides the retained value. A missing
replacement time preserves it. Default historical stores leave values unchanged.
The completed normalized JSONL cache serializes these existing patch values.

Array identity follows JSON Patch semantics: insertion shifts later timestamps
right, removal shifts them left, and a whole-array replacement clears retained
metadata. This covers AmpResume's repeated remove-at-zero history reset. The
bounded ordered map stores at most 100,000 entry timestamps.
