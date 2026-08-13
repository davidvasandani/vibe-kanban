# Clarifications: Move deployment refresh

No user-blocking questions remain.

1. The requested Deploy Status “accordion” deliberately replaces the existing
   permanent, non-collapsible status row with the shared right-sidebar
   disclosure-section behavior.
2. Revision and deployment age stay in the section header, preserving useful
   status while collapsed.
3. Refresh is the existing newer-web-deployment reload action. Native desktop
   Update remains beneath the AppBar utilities when available.
4. Refresh is a header action with independent event handling, so it neither
   requires expansion nor changes the accordion state.
5. This relocation applies to the desktop AppBar/workspace sidebar only. The
   separate mobile navbar deployment identity and detection behavior remains
   unchanged.
