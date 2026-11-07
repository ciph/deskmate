import type { AiUsage } from "../../lib/settings";
import type { Dict } from "../../lib/i18n";
import type { UsageCloud } from "./model";

export function usageCloud(usage: AiUsage, t: Dict): UsageCloud {
  if (!("kind" in usage)) return {
    label: t.aiUsageWeeklyRemaining, percent: usage.remainingPct,
    amount: `¥${(usage.remainingCny / 10000).toFixed(2)}`, note: "",
  };
  if (usage.kind === "openrouter") return {
    label: t.aiUsageKeyRemaining,
    percent: usage.limitUsd !== null && usage.limitUsd > 0 && usage.remainingUsd !== null
      ? Math.round(Math.min(100, Math.max(0, usage.remainingUsd / usage.limitUsd * 100))) : null,
    amount: usage.remainingUsd === null ? "—" : `$${usage.remainingUsd.toFixed(2)}`,
    note: usage.limitUsd === null ? t.aiUsageNoKeyLimit : "",
  };
  if (usage.kind === "deepseek") return {
    label: t.aiUsageAccountBalance, percent: null,
    amount: usage.balances.map(balance => `${balance.currency === "CNY" ? "¥" : "$"}${balance.totalBalance}`).join(" · ") || "—",
    note: "",
  };
  return { label: t.aiUsageAccountBalance, percent: null, amount: "—", note: t.aiUsageAccountUnavailable };
}

export function usageError(error: unknown, t: Dict): string {
  const message = (error instanceof Error ? error.message : String(error)).replace(/^Error:?\s*/i, "");
  if (message === "empty_key") return t.aiUsageMissingKey;
  if (message === "deepseek_auth_failed") return t.aiUsageKeyInvalid;
  if (["unauthorized", "status:401", "status:403"].includes(message)) return t.aiUsageUnauthorized;
  return t.aiUsageUnavailable;
}
