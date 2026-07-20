# Data Model

No data model changes. The existing in-memory `QueuedMessageService` remains one
optional `QueuedMessage` per session, and the existing `DraftFollowUpData`
continues to carry message content plus executor configuration.
