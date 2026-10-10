import { useRef, useState } from "react";
import type { Dict } from "../lib/i18n";
import { verifyError } from "../lib/i18n";
import {
  modelsMatchVerification,
  verifyApiKey,
  type ProviderModel,
  type Settings,
} from "../lib/settings";
import type { PersistSettings } from "./settingsPrimitives";

export type ProviderVerifyResult = {
  readonly ok: boolean;
  readonly message: string;
};

type ProviderActionsInput = {
  readonly settings: Settings;
  readonly persist: PersistSettings;
  readonly refreshModels: () => Promise<ProviderModel[]>;
  readonly onProviderVerified: (providerId: string) => void;
  readonly t: Dict;
};

function safeVerifyMessage(t: Dict, error: Error | string): string {
  const code = (error instanceof Error ? error.message : error).replace(/^Error:?\s*/i, "");
  if (code.startsWith("model_probe_failed:")) {
    return t.aiProviderManualVerifyFailed(code.slice("model_probe_failed:".length));
  }
  if (code === "manual_models_protocol") return t.aiProviderManualProtocol;
  if (["invalid_manual_model", "too_many_manual_models"].includes(code)) {
    return t.aiProviderManualInvalid;
  }
  if (
    [
      "empty_key",
      "bad_url",
      "unauthorized",
      "not_found",
      "no_models",
      "models_unavailable",
    ].includes(code) ||
    code.startsWith("status:") ||
    code.startsWith("network:")
  ) {
    return verifyError(t, code);
  }
  return t.aiUnreachable;
}

export function useAiProviderActions({
  settings,
  persist,
  refreshModels,
  onProviderVerified,
  t,
}: ProviderActionsInput) {
  const [verifyingProviderId, setVerifyingProviderId] = useState<string | null>(
    null,
  );
  const [verifyResults, setVerifyResults] = useState<
    Readonly<Record<string, ProviderVerifyResult>>
  >({});
  const operationRequest = useRef(0);

  const beginOperation = () => {
    operationRequest.current += 1;
    return operationRequest.current;
  };
  const isCurrentOperation = (requestId: number) =>
    operationRequest.current === requestId;

  const verifyProvider = async (providerId: string) => {
    const requestId = beginOperation();
    setVerifyingProviderId(providerId);
    setVerifyResults((current) => {
      const next = { ...current };
      delete next[providerId];
      return next;
    });
    try {
      const provider = settings.providers.find((entry) => entry.id === providerId);
      if (!provider) throw new Error("provider_missing");
      await persist(settings);
      if (!isCurrentOperation(requestId)) return;
      const count = await verifyApiKey(
        provider.id,
        provider.baseUrl,
        provider.apiKey,
        provider.manualModelIds,
      );
      if (!isCurrentOperation(requestId)) return;
      const refreshed = await refreshModels();
      if (!isCurrentOperation(requestId)) return;
      if (!modelsMatchVerification(refreshed, provider.sidecarId, count)) {
        throw new Error("models_unavailable");
      }
      onProviderVerified(provider.id);
      setVerifyResults((current) => ({
        ...current,
        [providerId]: { ok: true, message: t.verifyOk(count) },
      }));
    } catch (error) {
      const normalizedError =
        error instanceof Error ? error : String(error);
      if (isCurrentOperation(requestId)) {
        setVerifyResults((current) => ({
          ...current,
          [providerId]: {
            ok: false,
            message: safeVerifyMessage(t, normalizedError),
          },
        }));
      }
    } finally {
      if (isCurrentOperation(requestId)) setVerifyingProviderId(null);
    }
  };

  const clearOperationState = () => {
    operationRequest.current += 1;
    setVerifyingProviderId(null);
    setVerifyResults({});
  };

  return {
    clearOperationState,
    operationBusy: verifyingProviderId !== null,
    verifyProvider,
    verifyResultFor: (providerId: string): ProviderVerifyResult | null =>
      verifyResults[providerId] ?? null,
    verifyingProviderId,
  };
}
