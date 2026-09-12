import { useEffect } from 'react';

const BASE_TITLE = 'Vibe Kanban';

/**
 * Sets the document title from the first meaningful candidate.
 * Multiple callers can coexist — the most specific (deepest) component wins
 * because React runs child effects after parent effects.
 *
 * No cleanup is performed on unmount so that a parent-level caller
 * (e.g. the legacy ProjectProvider) provides a stable fallback without
 * competing with page-level callers.
 */
export function usePageTitle(...candidates: (string | null | undefined)[]) {
  const title =
    candidates.find((candidate) => candidate?.trim())?.trim() ?? BASE_TITLE;

  useEffect(() => {
    document.title = title;
  }, [title]);
}
