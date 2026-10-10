import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";
import { findSourceBinary } from "./prepare-opencode";
import { buildPermissionMap, validatePermissionMap } from "./sidecar-harness/permissions";
import { requestJson } from "./sidecar-harness/transport";

describe("OpenCode sidecar preparation", () => {
  test("packages a real Windows executable instead of the npm error shim", async () => {
    const sourceBinary = await findSourceBinary();
    const bytes = await readFile(sourceBinary);

    if (process.platform === "win32") {
      expect(bytes.subarray(0, 2)).toEqual(Buffer.from([0x4d, 0x5a]));
    }
  });

  test("declares a deny-by-default permission map without external deployment tools", () => {
    const permission = buildPermissionMap();
    const errors = validatePermissionMap(permission);

    expect(errors).toEqual([]);
    expect(permission["*"]).toBe("deny");
    expect(Object.values(permission)).not.toContain("allow");
  });

  test("reports slow discovery requests without leaking query details", async () => {
    let message = "";
    try {
      await requestJson({
        baseUrl: "http://127.0.0.1",
        path: "/config?opaque=canary",
        timeoutSeconds: 0.01,
        runner: () => new Promise<never>(() => {}),
      });
    } catch (error) {
      if (!(error instanceof Error)) throw error;
      message = error.message;
    }

    expect(message).toContain("/config request failed: timeout");
    expect(message).not.toContain("opaque");
    expect(message).not.toContain("canary");
  });
});
