interface WorkspaceContextBarVisibilityInput {
  isResponsiveMobile: boolean;
  isRealMobileDevice: boolean;
}

export function shouldRenderWorkspaceContextBar({
  isResponsiveMobile,
  isRealMobileDevice,
}: WorkspaceContextBarVisibilityInput): boolean {
  return !isResponsiveMobile && !isRealMobileDevice;
}
