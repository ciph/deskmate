export function containsProviderApiKey(text: string): boolean {
  if (!/(cc\s*switch|opencode)/iu.test(text)) return false;
  return (
    /\bsk-[a-z0-9_-]{8,}/iu.test(text) ||
    /\bbearer\s+[a-z0-9._-]{8,}/iu.test(text) ||
    /["']?(?:api[\s_-]?key|token|secret)["']?\s*[:=]\s*["']?[a-z0-9._-]{8,}/iu.test(
      text,
    )
  );
}
