import { getAiUsage, type AiProvider, type AiUsage } from "../lib/settings";
import type { Dict } from "../lib/i18n";

type UsageView =
  | { readonly kind: "loading" | "missing-key" | "unavailable" | "invalid-key" | "unauthorized" }
  | { readonly kind: "ready"; readonly usage: AiUsage };
interface CachedUsage {
  readonly provider: AiProvider;
  view: UsageView;
  fetchedAt: number | null;
  pending: Promise<UsageView> | null;
}

export function sameUsageProvider(a: AiProvider | null, b: AiProvider | null): boolean {
  return a === b || (a !== null && b !== null && a.id === b.id && a.baseUrl === b.baseUrl
    && a.apiKey === b.apiKey && a.manualModelIds === b.manualModelIds && a.sidecarId === b.sidecarId);
}

/** One in-memory account snapshot; reading it is the only refresh trigger. */
export class PetAiUsageCache {
  private entry: CachedUsage | null = null;

  constructor(private readonly fetchUsage = getAiUsage, private readonly now = Date.now) {}

  read(provider: AiProvider | null): { view: UsageView; refreshed: Promise<UsageView> | null } {
    if (!provider?.apiKey.trim()) {
      this.entry = null;
      return { view: { kind: "missing-key" }, refreshed: null };
    }
    if (!sameUsageProvider(this.entry?.provider ?? null, provider)) this.entry = null;
    const entry = this.entry ??= { provider, view: { kind: "loading" }, fetchedAt: null, pending: null };
    const now = this.now();
    const fresh = entry.fetchedAt !== null && now >= entry.fetchedAt && now - entry.fetchedAt < 60_000
      && new Date(now).toDateString() === new Date(entry.fetchedAt).toDateString();
    if (!fresh && entry.pending === null) {
      entry.pending = this.fetchUsage(provider.id).then((usage): UsageView => {
        entry.fetchedAt = this.now();
        return { kind: "ready", usage };
      }, (error: unknown): UsageView => {
        entry.fetchedAt = null;
        const message = (error instanceof Error ? error.message : String(error)).replace(/^Error:?\s*/i, "");
        return { kind: message === "deepseek_auth_failed" ? "invalid-key"
          : ["unauthorized", "status:401", "status:403"].includes(message) ? "unauthorized" : "unavailable" };
      }).then((view) => {
        entry.view = view;
        entry.pending = null;
        return view;
      });
    }
    return { view: entry.view, refreshed: entry.pending };
  }
}

export function petAiUsageLines(view: UsageView, t: Dict): readonly [string, string] {
  if (view.kind !== "ready") {
    const status = view.kind === "loading" ? t.aiUsageLoading
      : view.kind === "missing-key" ? t.aiUsageMissingKey
      : view.kind === "invalid-key" ? t.aiUsageKeyInvalid
      : view.kind === "unauthorized" ? t.aiUsageUnauthorized : t.aiUsageUnavailable;
    return [status, t.petAiUsageSettingsHint];
  }
  const usage = view.usage;
  if (!("kind" in usage)) {
    return [
      `${t.aiUsageWeeklyRemaining}  ${usage.remainingPct}% · ¥${(usage.remainingCny / 10000).toFixed(0)} / ¥${(usage.limitCny / 10000).toFixed(0)}`,
      `${t.aiUsagePlatformToday}  ¥${(usage.todayCostCny / 10000).toFixed(2)} · ${t.aiUsageRequests(usage.todayRequests)}`,
    ];
  }
  if (usage.kind === "openrouter") {
    return [usage.limitUsd === null && usage.remainingUsd === null ? t.aiUsageNoKeyLimit
      : `${t.aiUsageKeyRemaining}  ${usage.remainingUsd === null ? "—" : `$${usage.remainingUsd.toFixed(2)}`}`,
    `${t.aiUsagePlatformToday}  $${usage.todayCostUsd.toFixed(2)}`];
  }
  const local = usage.localAvailable
    ? `${t.aiUsageYumeToday}  ${t.aiUsageTokens(usage.todayTokens)} · ${t.aiUsageRequests(usage.todayRequests)}`
    : t.aiUsageLocalUnavailable;
  if (usage.kind === "deepseek") {
    const balances = usage.balances.map(balance => `${balance.currency === "CNY" ? "¥" : "$"}${balance.totalBalance}`).join(" · ");
    return [`${t.aiUsageAccountBalance}  ${balances || "—"}`, local];
  }
  return [local, t.aiUsageLocalHint];
}
