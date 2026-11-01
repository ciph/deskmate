import { expect, test } from "bun:test";
import { canControlRunningReply, validateSharedConversation, type ConversationAccess } from "./sharedConversation";
import { nativeHistoryFixture } from "../testing/historyCatalogFixtures";

const light: ConversationAccess = {
  key: "native:one:a:ses-same",
  identity: { kind: "native", sidecarId: "one", directory: "C:/project-a", sessionId: "ses-same" },
  capabilities: { send: true, readOnlyReason: null },
};

test("a native handoff preserves directory and native session identity", () => {
  expect(validateSharedConversation(light, { ...light })?.sessionId).toBe("ses-same");
  expect(validateSharedConversation(light, { ...light })?.directory).toBe("C:/project-a");
});

test("colliding bare IDs from another project cannot receive a send", () => {
  expect(validateSharedConversation(light, {
    ...light, identity: { ...light.identity, kind: "native", sidecarId: "one", directory: "C:/project-b", sessionId: "ses-same" },
  })).toBeNull();
});

test("fresh host capabilities deny workbench, active, unavailable and legacy writes", () => {
  for (const reason of ["workbench_owned", "agent_active", "unavailable", "archived"]) {
    expect(validateSharedConversation(light, { ...light, capabilities: { send: false, readOnlyReason: reason } })).toBeNull();
  }
  const legacy: ConversationAccess = { ...light, identity: { kind: "legacy", historyId: "old" } };
  expect(validateSharedConversation(legacy, legacy)).toBeNull();
});

test("switching catalog key prevents stale authorization from resuming the old view", () => {
  expect(validateSharedConversation({ ...light, key: "native:other" }, light)).toBeNull();
});

test("local reply controls require exact identity and only the active-task restriction", () => {
  const row = nativeHistoryFixture("ses_control");
  const running = { ...row, runtime: "running" as const,
    capabilities: { ...row.capabilities, send: false, readOnlyReason: "active_task" } };
  expect(canControlRunningReply(running, running.key)).toBe(true);
  expect(canControlRunningReply(running, undefined)).toBe(false);
  expect(canControlRunningReply(running, `${running.key}:other-project`)).toBe(false);
  for (const denied of [
    { ...running, ownership: "workbench" as const },
    { ...running, ownership: "agent" as const },
    { ...running, archived: true },
    { ...running, tombstone: { requestedAt: 1, remoteDeleted: false } },
    { ...running, availability: "unavailable" as const },
    { ...running, runtime: "unknown" as const },
    { ...running, capabilities: { ...running.capabilities, readOnlyReason: "native_unavailable" } },
    { ...running, identity: { kind: "legacy" as const, historyId: "old" } },
  ]) expect(canControlRunningReply(denied, running.key)).toBe(false);
});
