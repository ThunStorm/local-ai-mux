export type CacheDiagnostics = {
  usage_source?: string | null;
  usage_scope?: string | null;
  usage_status?: string | null;
  reported_input_tokens?: number | null;
  reported_cached_input_tokens?: number | null;
};

// Only per-request upstream counts can describe a cache share. Old rows have no cache report.
export function cacheUsage(diagnostics?: CacheDiagnostics | null, legacyInput?: number | null) {
  const reported = diagnostics?.usage_source === "provider_reported"
    && (diagnostics.usage_scope === "request" || diagnostics.usage_scope === "last_turn")
    && (diagnostics.usage_status == null || diagnostics.usage_status === "provider_reported");
  const count = (value: number | null | undefined) => typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
  const input = reported ? count(diagnostics?.reported_input_tokens ?? legacyInput) : null;
  const candidate = reported ? count(diagnostics?.reported_cached_input_tokens) : null;
  const cached = input !== null && candidate !== null && candidate <= input ? candidate : null;
  return { reported, input, cached, percent: input !== null && input > 0 && cached !== null ? 100 * (cached / input) : null };
}
