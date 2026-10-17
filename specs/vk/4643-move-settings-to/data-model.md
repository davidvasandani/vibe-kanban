# Data Model: Settings drawer (client state only)

## SettingsDrawerState (zustand, `useSettingsDrawerStore`)
| Field | Type | Persisted | Notes |
|---|---|---|---|
| `isOpen` | `boolean` | no | True while the drawer component is mounted and visible. It starts closed on reload. |
| `width` | `number` (px) | yes: `localStorage['vibe.ui.settingsDrawerWidth']` | Defaults to 720. The stored value is the preference. The clamp is applied when rendering against the current viewport. |
| `requestClose` | `(() => void \| Promise<void>) \| null` | no | Registered by the mounted drawer. It runs the unsaved-changes guard. |

## Width bounds
- `SETTINGS_DRAWER_MIN_WIDTH = 520`
- `SETTINGS_DRAWER_MAX_WIDTH = 1200`
- `SETTINGS_DRAWER_APP_RESERVE = 720`: pixels always left for the app
- `clamp(w, vw) = max(MIN, min(w, MAX, vw − RESERVE))`. The result is always
  at least MIN.

No server, database or API changes.
