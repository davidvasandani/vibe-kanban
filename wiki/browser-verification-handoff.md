# Browser verification handoff

Workspace Browser launches headless Chromium and delivers both human and agent input through CDP. Taking control changes ownership in SessionRuntime, not the browser environment. Acquire/release/transfer do not navigate, reload, replace the profile or recreate the driver. Local keyboard-bridge remounts do not remount the remote page. The frontend shows JPEG screencast frames; it does not proxy challenge iframe postMessage traffic.

Cloudflare explicitly excludes automated browsers/frameworks from production challenge solving in its [supported environments](https://developers.cloudflare.com/cloudflare-challenges/reference/supported-browsers/). A headed display attached to an automated session does not establish provider support. Treat headless automation as a compatibility limitation, not proof of the sole rejection cause.

The Browser panel offers an explicit local-browser fallback. The link is HTTP(S) origin only: credentials, path, query and fragment are discarded because all can contain session material. It opens with noopener/noreferrer. Tauri's existing on_new_window handler opens the OS browser. The hosted session and lease remain unchanged and keep their normal expiry. Local cookies/cart are separate, so the human recreates the cart. No token, cookie or payment-data export is involved.

CDP console diagnostics are fixed classifications, not raw console arguments or object descriptions. Actual Network.loadingFailed events add classified network observations to the same 100-entry ring. Unknown error text is suppressed; cancellation is distinguished from failure. Cross-origin resource timing status 0 is not a failed-request event. Turnstile unexpected-source and missing-widget warnings are classified observations with an unconfirmed cause, not a reason to manipulate widgets or retry challenges.

Tests cover safe links, unchanged hosted input, stale session isolation, control exclusivity and bounded secret-free diagnostics. `scripts/fixtures/browser-verification.html` uses Cloudflare's public always-pass test key. Run the opt-in Rust test `live_test_widget_survives_control_handoff` with local Chromium and network access. It checks page identity, cart marker and provider test-widget state through takeover/release/reacquire. It does not establish production challenge compatibility. Real checkout validation requires a human and must stop before purchase.

## Contributed by

- vk/aa2f-fix-cloudflare-v, workspace aa2f8491-4535-4f06-a49f-265795d01056
