# Research

The legacy UI scratch deliberately uses UUID 1 for all users. Extending that record with personal choices would preserve the bug. Dedicated UI scratch records avoid a backend migration and isolate writes from legacy autosave. Local scratch is server-backed; remote scratch is localStorage-backed. No new server capability or authentication boundary is claimed: scratch endpoints retain the existing trusted-instance access model. Defaults are preferences, not private data.

Search is intentionally transient; project-specific tags are dropped when applying a default to another project. Assignee Me is retained as __self__. Do not migrate shared legacy choices into user records.
