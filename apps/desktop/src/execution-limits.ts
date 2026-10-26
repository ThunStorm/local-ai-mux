export const MAX_CLI_IDLE_SECONDS = 1200;
export function parseCliIdleSeconds(value: string): number | null {
  const seconds = Number(value);
  return value.trim() !== "" && Number.isInteger(seconds) && seconds >= 30 && seconds <= MAX_CLI_IDLE_SECONDS ? seconds : null;
}
