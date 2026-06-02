import { z } from "zod/v4";

// --- Daemon JSON response envelope ---

export const daemonErrorEnvelopeSchema = z.object({
  type: z.literal("error"),
  error: z.string(),
});

export const daemonSuccessEnvelopeSchema = z.object({
  ok: z.literal(true),
  data: z.unknown().optional(),
});

// A daemon command response may be either the error envelope or the success envelope,
// or a raw payload. We validate the common patterns.
export const daemonResponseSchema = z.union([
  daemonErrorEnvelopeSchema,
  daemonSuccessEnvelopeSchema,
]);

// --- WebUI session (daemon webui-start response) ---

export const webuiSessionSchema = z.object({
  base_url: z.string().min(1),
  token: z.string().min(1),
});

export type WebuiSession = z.infer<typeof webuiSessionSchema>;

// --- Structured error extraction ---

export function extractStructuredError(payload: unknown): string | null {
  const parsed = daemonErrorEnvelopeSchema.safeParse(payload);
  if (parsed.success) return parsed.data.error;

  // Also check the { ok: false, error: "..." } pattern from batch sub-commands
  const batchErr = z
    .object({ ok: z.literal(false), error: z.string() })
    .safeParse(payload);
  if (batchErr.success) return batchErr.data.error;

  return null;
}

// --- Generic JSON output parser ---

export function parseDaemonJson(raw: string): unknown {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (cause) {
    throw new Error(
      `Failed to parse daemon JSON: ${cause instanceof Error ? cause.message : cause}`,
    );
  }

  const structured = extractStructuredError(parsed);
  if (structured) throw new Error(structured);

  // Unwrap the { ok: true, data: ... } envelope when present
  const envelope = daemonSuccessEnvelopeSchema.safeParse(parsed);
  if (envelope.success) return envelope.data.data;

  return parsed;
}
