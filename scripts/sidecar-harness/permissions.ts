import { expectJsonObject, HarnessError, isJsonObject, type JsonObject } from "./types";

export const EXPECTED_OPENCODE_VERSION = "1.18.21";

const deniedToolIds = ["bash", "edit", "write", "patch", "external_directory", "task"] as const;

export function buildPermissionMap(): Record<string, "allow" | "deny"> {
  return {
    "*": "deny",
    bash: "deny",
    edit: "deny",
    write: "deny",
    patch: "deny",
    external_directory: "deny",
    task: "deny",
  };
}

export function buildSidecarConfig(providerBaseUrl = "http://127.0.0.1:9"): JsonObject {
  return {
    $schema: "https://opencode.ai/config.json",
    permission: buildPermissionMap(),
    provider: {
      yume: {
        npm: "@ai-sdk/openai-compatible",
        name: "YUME",
        options: { baseURL: providerBaseUrl },
        models: { "model-a": { name: "Model A" } },
      },
    },
  };
}

export function validatePermissionMap(value: unknown): readonly string[] {
  const map = isJsonObject(value) ? value : {};
  const errors: string[] = [];
  if (map["*"] !== "deny") errors.push("permission.* must deny");
  for (const id of deniedToolIds) {
    if (map[id] !== "deny") errors.push(`permission.${id} must deny`);
  }
  const extraAllows = Object.entries(map)
    .filter(([, action]) => action === "allow")
    .map(([id]) => id);
  if (extraAllows.length > 0) errors.push(`unexpected allow: ${extraAllows.join(",")}`);
  return errors;
}

export function assertHealth(value: unknown): JsonObject {
  const health = expectJsonObject(value, "health");
  if (health.healthy !== true || health.version !== EXPECTED_OPENCODE_VERSION) {
    throw new HarnessError(`unexpected health: ${JSON.stringify(health)}`);
  }
  return health;
}
