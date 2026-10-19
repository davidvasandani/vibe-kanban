# Research
The local action enum has click, mouse_move and key but no scrolling. CDP mouseWheel is the appropriate existing input mechanism, preserving hit testing for nested scroll containers. DOM gesture coordinates are CSS pixels and must be scaled to the screencast viewport. React wheel delegation can be passive, so attach a native listener with passive:false. No new dependencies.
