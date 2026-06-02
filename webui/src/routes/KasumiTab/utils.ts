export function parseUnsignedInput(value: string, label: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error(`${label} cannot be empty`);
  }
  const parsed = /^0x/i.test(trimmed)
    ? Number.parseInt(trimmed, 16)
    : Number.parseInt(trimmed, 10);

  if (!Number.isFinite(parsed) || Number.isNaN(parsed) || parsed < 0) {
    throw new Error(`Invalid ${label}: ${value}`);
  }

  return parsed;
}
