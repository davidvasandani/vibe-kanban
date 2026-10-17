# Independent Review: Stable Earlier-History Loading Geometry

## Review Loop

The independent Codex CLI reviewed the complete tracked and untracked diff four
times.

1. It found that `min-height` was not invariant for wrapped translations or
   enlarged text. The implementation changed to responsive intrinsic sizing
   layers.
2. It found that hiding a persistent focused button would regress keyboard and
   assistive-technology focus. The active button now unmounts during loading;
   only noninteractive, `aria-hidden` sizing copies persist.
3. It found that retry cleared the error row and could shrink the control. The
   error row now remains in layout and becomes invisible when inactive.
4. It found that an invisible sizing skeleton still ran pulse animations. Hidden
   sizing copies are now static; only active loading feedback animates.

## Final Result

Final review: “The extracted control preserves existing pagination behavior
while maintaining stable intrinsic geometry across idle, loading, and error
states. No actionable correctness regressions were identified in the changed
code.”
