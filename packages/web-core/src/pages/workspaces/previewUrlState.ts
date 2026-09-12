const PREVIEW_TRANSPORT_PARAMS = [
  '_refresh',
  '_vk_workspace',
  '_vk_execution',
  '_vk_generation',
] as const;

function removePreviewTransportParams(url: URL): void {
  for (const param of PREVIEW_TRANSPORT_PARAMS) {
    url.searchParams.delete(param);
  }
}

export function stripPreviewTransportParams(rawUrl: string): string | null {
  try {
    const url = new URL(rawUrl);
    removePreviewTransportParams(url);
    return url.toString();
  } catch {
    return null;
  }
}

export function getPreviewRoute(rawUrl: string): string | null {
  try {
    const url = new URL(rawUrl);
    removePreviewTransportParams(url);
    return `${url.pathname}${url.search}${url.hash}`;
  } catch {
    return null;
  }
}

export function applyPreviewRoute(
  baseUrl: string,
  route: string | null
): string | null {
  try {
    const base = new URL(baseUrl);
    removePreviewTransportParams(base);

    if (!route) {
      return base.toString();
    }

    if (!route.startsWith('/') || route.startsWith('//')) {
      return null;
    }

    const restored = new URL(route, base);
    if (restored.origin !== base.origin) {
      return null;
    }
    removePreviewTransportParams(restored);
    return restored.toString();
  } catch {
    return null;
  }
}
