export class AppError extends Error {
  constructor(
    public message: string,
    public code?: number,
  ) {
    super(message);
    this.name = "AppError";
  }
}

export function getErrorMessage(
  e: unknown,
  fallback = "Unknown error",
): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  return fallback;
}
