# VK Browser scrolling

Add scrolling to the streamed browser so controlling users can read pages beyond the visible frame. Support one-finger vertical/horizontal swipes and desktop mouse-wheel/trackpad input. Forward through the existing guarded browser action protocol with a new Scroll variant, using remote coordinates and scaled deltas. Swipes must not click or open the keyboard; taps keep the existing behavior. Ignore letterbox starts, read-only input, cancelled gestures and gestures from an earlier session/control generation. Prevent the host page from scrolling while interacting with the controlled remote frame. Extend the typed backend protocol and regenerate shared types; no hosting changes.

Verify scroll direction, scaling, wheel delta modes, tap/swipe separation, cancellation and control/session transitions in rendered component tests; run frontend checks and independent review before merging. Physical phone verification remains distinct from synthetic tests.
