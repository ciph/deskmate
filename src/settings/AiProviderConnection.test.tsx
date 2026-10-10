import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import * as tauriCore from "@tauri-apps/api/core";
import * as tauriEvent from "@tauri-apps/api/event";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AiProvider, Settings } from "../lib/settings";
import {
  legacySettingsFixture,
  multiProviderSettingsFixture,
} from "../testing/settingsFixtures";

const invoke = mock<(command: string, args?: unknown) => Promise<unknown>>(
  (command) => defaultInvoke(command),
);
const emit = mock<(event: string, payload?: unknown) => Promise<void>>(() =>
  Promise.resolve(),
);
const listen = mock<(event: string, handler: unknown) => Promise<() => void>>(() =>
  Promise.resolve(() => undefined),
);

mock.module("@tauri-apps/api/core", () => ({ ...tauriCore, invoke }));
mock.module("@tauri-apps/api/event", () => ({ ...tauriEvent, emit, listen }));

const SettingsApp = (await import("./SettingsApp")).default;

const originalFetch = globalThis.fetch;

const settingsFixture = legacySettingsFixture();

function defaultInvoke(command: string): Promise<unknown> {
  if (command === "get_pet_visibility_error") return Promise.resolve(null);
  if (command === "get_settings") return Promise.resolve(settingsFixture);
  if (command === "sidecar_base_url") return Promise.resolve("http://127.0.0.1:48111");
  if (command === "verify_api_key") return Promise.resolve(1);
  if (command === "fetch_ai_usage") return Promise.reject(new Error("unauthorized"));
  return Promise.resolve(undefined);
}

afterEach(() => {
  cleanup();
  globalThis.fetch = originalFetch;
});

beforeEach(() => {
  invoke.mockClear();
  invoke.mockImplementation((command) => defaultInvoke(command));
  emit.mockClear();
  listen.mockClear();
  const fetchResponse = mock(() =>
    Promise.resolve(new Response(JSON.stringify(modelCatalogFixture), { status: 200 })),
  );
  globalThis.fetch = Object.assign(fetchResponse, {
    preconnect: mock(() => undefined),
  });
});

const modelCatalogFixture = {
  providers: [{
    id: "yume", name: "YUME",
    models: { "gpt-5.4-mini": { id: "gpt-5.4-mini", name: "gpt-5.4-mini" } },
  }],
};

async function openAiSettings() {
  const user = userEvent.setup();
  render(<SettingsApp />);
  await user.click(await screen.findByRole("button", { name: /AI/ }));
  return user;
}

describe("Provider connection", () => {
  test("shares a multi-provider settings fixture that keeps legacy routing fields", () => {
    const settings: Settings = multiProviderSettingsFixture();
    const providers: readonly AiProvider[] = settings.providers;

    expect(settings.providerId).toBe("yume");
    expect(settings.modelId).toBe("gpt-5.4-mini");
    expect(settings.baseUrl).toBe("https://ai-gateway.kurogames.com");
    expect(settings.apiKey).toBe("configured-key");
    expect(settings.activeProviderId).toBe("provider-kuro");
    expect(providers).toEqual([
      {
        id: "provider-kuro",
        sidecarId: "yume",
        label: "Kuro",
        baseUrl: "https://ai-gateway.kurogames.com",
        apiKey: "configured-key",
      },
      {
        id: "provider-omo-kuro",
        sidecarId: "yume-2",
        label: "OMO Kuro",
        baseUrl: "https://omo-kuro.example.test/v1",
        apiKey: "omo-configured-key",
      },
    ]);
  });
  test("persists an edited provider before verifying its credentials", async () => {
    const user = await openAiSettings();
    await user.click(screen.getByRole("button", { name: "添加供应商" }));

    const providerCard = screen.getByRole("article", { name: "供应商 2" });
    const baseUrlInput = within(providerCard).getByLabelText("Base URL · 供应商 2");
    const apiKeyInput = within(providerCard).getByLabelText("API Key · 供应商 2");
    await user.click(baseUrlInput);
    await user.paste("https://frontier.example.test/v1");
    await user.click(apiKeyInput);
    await user.paste("frontier-secret");
    await user.click(within(providerCard).getByRole("button", { name: "验证并连接" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "set_settings",
        expect.objectContaining({
          settings: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                sidecarId: "yume-2",
                baseUrl: "https://frontier.example.test/v1",
                apiKey: "frontier-secret",
              }),
            ]),
          }),
        }),
      );
      expect(invoke).toHaveBeenCalledWith(
        "verify_api_key",
        expect.objectContaining({
          baseUrl: "https://frontier.example.test/v1",
          apiKey: "frontier-secret",
        }),
      );
    });

    const relevant = invoke.mock.calls
      .map(([command]) => command)
      .filter((command) => command === "set_settings" || command === "verify_api_key");
    expect(relevant.slice(-2)).toEqual(["set_settings", "verify_api_key"]);
    expect(document.body.textContent).not.toContain("frontier-secret");
  });
  test("locks routing during verification and reports a successful local connection", async () => {
    let resolveVerification: ((count: number) => void) | undefined;
    invoke.mockImplementation((command) => {
      if (command === "verify_api_key") {
        return new Promise<number>((resolve) => {
          resolveVerification = resolve;
        });
      }
      return defaultInvoke(command);
    });

    const user = await openAiSettings();
    await user.click(screen.getByRole("button", { name: "Kuro" }));
    const providerCard = await screen.findByRole("article", { name: "Kuro" });
    await user.click(within(providerCard).getByRole("button", { name: "验证并连接" }));
    await screen.findByRole("button", { name: "验证中…" });
    expect(screen.getByRole("button", { name: "添加供应商" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("combobox")).toHaveProperty("disabled", true);
    await act(async () => resolveVerification?.(1));

    await waitFor(() => {
      expect(within(providerCard).getByRole("status").textContent).toContain("已连接，可选择模型");
    });
  });


  test("connects using the bundled engine without any deployment command or UI", async () => {
    const user = await openAiSettings();
    await user.click(screen.getByRole("button", { name: "Kuro" }));
    await user.click(screen.getByRole("button", { name: "验证并连接" }));
    await screen.findByText("已连接，可选择模型 (1)");
    expect(screen.queryByRole("button", { name: "部署" })).toBeNull();
    expect(screen.queryByRole("button", { name: "一键部署" })).toBeNull();
    expect(screen.queryByLabelText("CC Switch")).toBeNull();
    expect(invoke.mock.calls.some(([command]) => /ccswitch|deploy_local/.test(command))).toBe(false);
    expect(emit).not.toHaveBeenCalled();
  });

  test("shows verification failure safely and allows a retry", async () => {
    invoke.mockImplementation((command) => command === "verify_api_key"
      ? Promise.reject(new Error("synthetic-private-key-DO-NOT-RENDER"))
      : defaultInvoke(command));
    const user = await openAiSettings();
    await user.click(screen.getByRole("button", { name: "Kuro" }));
    const card = screen.getByRole("article", { name: "Kuro" });
    await user.click(within(card).getByRole("button", { name: "验证并连接" }));
    await waitFor(() => expect(within(card).getByRole("alert")).toBeDefined());
    expect(document.body.innerHTML).not.toContain("synthetic-private-key-DO-NOT-RENDER");
    expect(within(card).getByRole("button", { name: "验证并连接" })).toHaveProperty("disabled", false);
  });
});
