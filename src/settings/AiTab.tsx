import { AiUsage } from "./AiUsage";
import { AiProviderList } from "./AiProviderList";
import { displayProviderLabel } from "./aiProviderModel";
import { groupModelsByManufacturer } from "./modelManufacturer";
import {
  Row,
  type PersistSettings,
  type ReplaceSettings,
  type TabProps,
} from "./settingsPrimitives";
import { useAiTabController } from "./useAiTabController";

type AiTabProps = TabProps & {
  readonly replace: ReplaceSettings;
  readonly persist: PersistSettings;
};

export function AiTab({ settings, replace, persist, t }: AiTabProps) {
  const controller = useAiTabController({ settings, replace, persist, t });
  return (
    <>
      <AiProviderList
        settings={settings}
        replace={replace}
        t={t}
        onChange={controller.clearVerificationState}
        onVerify={(providerId) => void controller.verifyProvider(providerId)}
        verifyingProviderId={controller.verifyingProviderId}
        verifyResultFor={controller.verifyResultFor}
        operationBusy={controller.operationBusy}
      />

      <Row label={t.model}>
        <select
          className="set-select"
          value={controller.currentModelValue}
          disabled={controller.operationBusy}
          onChange={(e) => controller.pickModel(e.target.value)}
        >
          <option value="">{t.modelDefault}</option>
          {controller.groups.flatMap((g) =>
            groupModelsByManufacturer(g.models, t.modelOtherManufacturer).map(
              (manufacturer) => (
                <optgroup
                  key={`${g.providerId}/${manufacturer.label}`}
                  label={`${g.label} · ${manufacturer.label}`}
                >
                  {manufacturer.models.map((m) => (
                    <option
                      key={`${m.sidecarId}/${m.modelId}`}
                      value={`${m.sidecarId}/${m.modelId}`}
                    >
                      {m.modelName}
                    </option>
                  ))}
                </optgroup>
              ),
            ),
          )}
        </select>
      </Row>
      {controller.failed && (
        <p className="set-note set-note-error">{t.aiUnreachable}</p>
      )}

      <div className="set-ai-usage-list">
        {settings.providers.map((provider, index) => (
          <AiUsage
            key={provider.id}
            enabled={Boolean(provider.apiKey.trim())}
            providerId={provider.id}
            label={displayProviderLabel(provider, index, t)}
            index={index}
            t={t}
          />
        ))}
      </div>
    </>
  );
}
