# Data model: Move deployment refresh

No backend or persisted domain model changes.

The UI consumes three existing values:

| Value | Type | Owner | Meaning |
| --- | --- | --- | --- |
| `appVersion` | `string \| null` | `useUserSystem` | Running revision or `dev` sentinel |
| `deploymentTimestamp` | `string \| null` | `useUserSystem` | Immutable build/publish timestamp |
| `deployUpdateAvailable` | `boolean` | `useDeployUpdateAvailable` | Running server revision differs from the page-load revision |

The only new persisted UI datum is the Deploy Status accordion's boolean
expanded state, stored through the existing collapsible-section localStorage
convention and a new typed `PERSIST_KEYS` entry.
