# Technical spec: suppress auto-fix for credit and usage errors

Task: 723d-don-t-run-auto-f

Failed coding-agent turns caused by exhausted credits, billing quota, or usage limits must not launch automatic code remediation. Inspect normalized failure evidence before reserving remediation capacity or creating/reusing issues and starting workspaces. Preserve ordinary failure remediation and existing recursion/deduplication safeguards. Match structured error types where available and explicit provider error text where needed, without classifying ordinary code failures merely mentioning usage as quota errors.

Acceptance: credit/usage failures produce no remediation issue/workspace; ordinary code failures continue through existing handling; regression tests cover structured and textual errors and false positives. Scope is Vibe Kanban source only; no deployment changes anticipated.

Independent review correction: Claude failed Result events were emitted as errors only for AmpResume. Preserve is_error results for both strategies, retain the provider errors array, and exercise the real ClaudeLogProcessor through the admission classifier. Update qa_mock construction for the added field. This changes failed-result diagnostic visibility but leaves ordinary assistant text unclassified.
