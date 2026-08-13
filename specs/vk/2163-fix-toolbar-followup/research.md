# Research

The initial layout puts leading navigation and tools in the same horizontal
scroller. A nonzero retained scroll position therefore clips whichever control
is first. Narrowing scroll ownership to the tools isolates that state from fixed
edge controls. CSS flexbox remains sufficient; no JavaScript measurement or new
dependency is warranted.
