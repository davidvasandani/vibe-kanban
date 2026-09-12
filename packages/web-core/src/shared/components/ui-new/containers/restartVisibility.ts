export function shouldShowRestartBanner(
  isLoading: boolean,
  isConnected: boolean
): boolean {
  return !isLoading && !isConnected;
}
