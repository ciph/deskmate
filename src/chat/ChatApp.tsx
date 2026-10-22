// allow: SIZE_OK — legacy chat composition root; Agent history behavior lives in useAgentHistoryView and this file only wires existing chat primitives.
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type DragEvent,
} from "react";
import { ToolApprovalCards } from "./ToolApprovalCards";
import { useToolPermissions } from "./useToolPermissions";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDirectory } from "@tauri-apps/plugin-dialog";
import {
  abortSession,
  confirmPromptSubmission,
  createSession,
  promptAsync,
  subscribeEvents,
  waitForServer,
  SessionAbortError,
  type OpenCodeEvent,
} from "../lib/opencode";
import { broadcastPetActivity } from "../lib/petState";
import { recordCompletedAiUsage } from "../lib/localAiUsage";
import { createChatPetActivity } from "./petActivity";
import { DEFAULT_PERSONA_ID } from "../pet/personaCatalog";
import {
  XIAOZHU_IDENTITY_REPLY,
  XIAOZHU_NAME_ORIGIN_LINES,
  isXiaozhuIdentityQuestion,
  isXiaozhuNameOriginQuestion,
  personaDisplayName,
  personalizePersonaCopy,
  resolvePersonaId,
  shouldResetSessionForPersona,
  userNameInstruction,
} from "./chatPersona";
import {
  composeSystemPrompt,
  memoryBlockForTurn,
} from "./memoryActions";
import {
  getSettings,
  onResourceError,
  onSettingsChanged,
  type Settings,
} from "../lib/settings";
import { memoryForgetConversation } from "../lib/memory";
import type { ThemeId } from "../settings/theme";
import { dict, type Lang } from "../lib/i18n";
import { AppIcon } from "../ui/AppIcon";
import {
  type HistorySession,
} from "../lib/history";
import {
  type ModelReadyAttachment,
  type OpenCodeFilePart,
} from "./attachments";
import { ArtifactCard } from "./ArtifactCard";
import { AttachmentTray } from "./AttachmentTray";
import { LocalResourceTray } from "./LocalResourceTray";
import { useLocalResources } from "./useLocalResources";
import { isMediaFile, prepareChatResources, type LocalResource } from "./localResources";
import { localResourceError } from "./localResourceCopy";
import { ChatText } from "./ChatText";
import { createStreamTextBuffer } from "./streamText";
import { ChatMessageRow, type TextChatMessage, type ToolActivity } from "./ChatMessageRow";
import { ChatNavigation } from "./ChatNavigation";
import { useWorklogChat, type ChatWorklogOperation } from "./useWorklogChat";
import { WorklogReceipt, worklogChatCopy } from "./WorklogReceipt";
import { buildWorklogSystemInstruction, newUserMessageId, registerWorklogTurn, WORKLOG_TOOLS } from "./worklogActions";
import { buildCurrentInformationInstruction } from "./currentInformation";
import { webSearchSites } from "./webSearchSites";
import { WorkspaceTask } from "./WorkspaceTask";
import { ModelPicker, WorkspacePicker } from "./ComposerPickers";
import { composerCopy, folderErrorCopy, modelErrorCopy } from "./composerCopy";
import { INHERIT_MODEL, getConversationModel, resolveConversationModel, setConversationModel, type ChatModelChoice, type ConversationModelSelection } from "../lib/conversationModel";
import { useAgentRun } from "./useAgentRun";
import { useAgentHistoryView } from "./useAgentHistoryView";
import { HistoryOrganizer } from "./HistoryOrganizer";
import { catalogAccess, catalogLoad, saveLocalMessages, type UnifiedHistoryRow } from "../lib/unifiedHistory";
import { validateSharedConversation, loadSharedConversation, conversationReadOnlyReason } from "./sharedConversation";
import { useHistoryOrganizerRequest } from "./useHistoryOrganizerRequest";
import { toOpenCodeToolPart } from "./toolPart";
import { containsProviderApiKey } from "./providerKeyGuard";
import { useChatAttachments, type LocalAttachmentArtifact } from "./useChatAttachments";
import type { PreparedModelAttachments } from "./useChatAttachmentsUtils";
import { userAvatarSource } from "../lib/userAvatar";
import { usePersonaAvatars } from "./usePersonaAvatar";
import { personaSystemMarker } from "./messagePersona";
import "./chat.css";

interface ArtifactChatMessage {
  id: string;
  role: "artifact";
  text: "";
  artifactId: string;
}

type ChatMessage = TextChatMessage | ArtifactChatMessage;

interface PersonaData {
  persona: string;
  skills?: string;
  placeholders: {
    streaming?: string;
    completed?: string;
    error?: string;
  } | null;
}

type Status = "booting" | "ready" | "busy" | "error";

type View = "chat" | "history";

const FIXED_REPLY_TYPING_DELAY_MS = 1_000;
const EMPTY_AVATAR_SOURCES: readonly string[] = [];
const EMPTY_OPERATIONS: readonly ChatWorklogOperation[] = [];
const EMPTY_PREPARED_ATTACHMENTS: PreparedModelAttachments = {
  fileParts: [],
  fallbackPrompt: null,
  shouldSendToModel: false,
};

function waitForDelay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => {
    globalThis.setTimeout(resolve, milliseconds);
  });
}

function settleToolActivity(activity: ToolActivity | undefined): ToolActivity | undefined {
  return typeof activity === "object" ? (activity.running ? { ...activity, running: false } : activity) : undefined;
}

function preserveWebSearchActivity(activity: ToolActivity | undefined): ToolActivity | undefined {
  return typeof activity === "object" ? activity : undefined;
}

function historyChatMessages<T extends Pick<HistorySession, "messages">>(session: T): ChatMessage[] {
  const result: ChatMessage[] = [];
  const assistants = new Map<string, TextChatMessage>();
  for (const [index, message] of session.messages.entries()) {
    const grouped = message.role === "assistant" && !message.localOnly && message.messageId;
    const existing = grouped ? assistants.get(grouped) : undefined;
    if (existing) { existing.text += message.text; continue; }
    const next: TextChatMessage = {
      id: grouped || message.partId || message.messageId || `history-${index}`,
      nativeMessageId: message.messageId,
      role: message.role, personaId: message.personaId, text: message.text,
      localOnly: message.localOnly, time: message.time,
    };
    if (grouped) assistants.set(grouped, next);
    result.push(next);
  }
  return result;
}

function runMatchesConversation(run: { sessionId: string | null; workspacePath: string } | null, entry: UnifiedHistoryRow | null): boolean {
  if (!run || entry?.identity.kind !== "native" || run.sessionId !== entry.identity.sessionId) return false;
  const normalize = (path: string) => {
    const slash = path.replace(/\\/g, "/").replace(/\/+$/, "");
    return /^[A-Za-z]:/.test(slash) || slash.startsWith("//") ? slash.toLowerCase() : slash;
  };
  return normalize(run.workspacePath) === normalize(entry.identity.directory);
}

export default function ChatApp() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [status, setStatus] = useState<Status>("booting");
  const [isPersonaTyping, setIsPersonaTyping] = useState(false);
  const [lang, setLang] = useState("zh-CN");
  const resourceLanguage: Lang = lang === "en-US" || lang === "ja-JP" || lang === "ko-KR" ? lang : "zh-CN";
  const [theme, setTheme] = useState<ThemeId>("dark");
  const [activePersonaId, setActivePersonaId] = useState(DEFAULT_PERSONA_ID);
  const personaAvatars = usePersonaAvatars(messages.flatMap(message =>
    isTextChatMessage(message) && message.personaId ? [message.personaId] : []));
  const [view, setView] = useState<View>("chat");
  const showOrganizer = useCallback(() => setView("history"), []);
  useHistoryOrganizerRequest(showOrganizer);
  const [catalogEntry, setCatalogEntry] = useState<UnifiedHistoryRow | null>(null);
  const [modelSelection, setModelSelection] = useState<ConversationModelSelection>(INHERIT_MODEL);
  const modelSelectionRef = useRef<ConversationModelSelection>(INHERIT_MODEL);
  const [resolvedModel, setResolvedModel] = useState<ChatModelChoice | null>(null);
  const [modelNotice, setModelNotice] = useState<string | null>(null);
  const modelRequestRef = useRef(0);
  const [settingsRevision, setSettingsRevision] = useState(0);
  const [pendingWorkspace, setPendingWorkspace] = useState<{ path: string; generation: number } | null>(null);
  const [openPicker, setOpenPicker] = useState<"folder" | "model" | null>(null);
  const composerAnchor = useRef<HTMLElement>(null);
  const folderLockedRef = useRef(true);
  const catalogEntryRef = useRef<UnifiedHistoryRow | null>(null);
  const viewGenerationRef = useRef(0);
  const localReplySnapshotRef = useRef(false);
  const [historyLoading, setHistoryLoading] = useState(false);
  const currentDirectory = catalogEntry?.identity.kind === "native" ? catalogEntry.identity.directory : undefined;
  const sessionDirectory = () => catalogEntryRef.current?.identity.kind === "native" ? catalogEntryRef.current.identity.directory : undefined;

  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const submittingRef = useRef(false);
  const [isDragActive, setIsDragActive] = useState(false);
  const [currentSessionId, setCurrentSessionId] = useState<string | null>(null);
  const [workbenchOpenError, setWorkbenchOpenError] = useState<{
    readonly sessionId: string;
    readonly reason: "missing" | "failed";
  } | null>(null);
  const openInWorkbench = useCallback(async (sessionId: string, directory?: string) => {
    setWorkbenchOpenError(null);
    try {
      await invoke("workbench_open_session", { sessionId, directory });
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      setWorkbenchOpenError({
        sessionId,
        reason: message === "history_native_missing" ? "missing" : "failed",
      });
    }
  }, []);
  const [isCancelling, setIsCancelling] = useState(false);
  /** Whether the current session's input is owned by the workbench (§8.1):
   * the light chat then defers its send/approve/cancel handlers for it. */
  const [sessionOwnedByWorkbench, setSessionOwnedByWorkbench] = useState(false);
  const [workbenchSessionStateUnknown, setWorkbenchSessionStateUnknown] =
    useState(false);
  useEffect(() => {
    let cancelled = false;
    let checking = false;
    let ownershipObserved = false;
    const check = async () => {
      if (!currentSessionId) {
        setSessionOwnedByWorkbench(false);
        setWorkbenchSessionStateUnknown(false);
        return;
      }
      if (checking) return;
      checking = true;
      try {
        const owned = await invoke<boolean>("workbench_session_owned", {
          sessionId: currentSessionId,
          directory: currentDirectory,
        });
        if (cancelled) return;
        if (owned) {
          ownershipObserved = true;
          setSessionOwnedByWorkbench(true);
          setWorkbenchSessionStateUnknown(false);
          return;
        }
        if (!ownershipObserved) {
          setSessionOwnedByWorkbench(false);
          setWorkbenchSessionStateUnknown(false);
          return;
        }
        const snapshot = await invoke<{ state: "busy" | "idle" | "unavailable" }>(
          "workbench_session_status",
          { sessionId: currentSessionId, directory: currentDirectory },
        );
        if (cancelled) return;
        if (snapshot.state === "unavailable") {
          setSessionOwnedByWorkbench(true);
          setWorkbenchSessionStateUnknown(true);
          return;
        }
        ownershipObserved = false;
        setSessionOwnedByWorkbench(false);
        setWorkbenchSessionStateUnknown(false);
        setStatus((current) => {
          if (snapshot.state === "busy") return "busy";
          return current === "busy" ? "ready" : current;
        });
      } catch {
        if (cancelled) return;
        if (ownershipObserved) {
          setSessionOwnedByWorkbench(true);
          setWorkbenchSessionStateUnknown(true);
        } else {
          setSessionOwnedByWorkbench(false);
        }
      } finally {
        checking = false;
      }
    };
    void check();
    // Ownership can change while we are already on this session (handoff from
    // this window, or the workbench hiding); re-check on every such change.
    const unlisten = listen("workbench://ownership-changed", () => void check());
    const timer = window.setInterval(() => void check(), 2_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      void unlisten.then((off) => off());
    };
  }, [currentSessionId, currentDirectory]);
  const worklog = useWorklogChat(currentSessionId);
  const [memoryNotice, setMemoryNotice] = useState<string | null>(null);
  const t = dict(lang);
  // Latest dict for use inside stable callbacks (SSE handler, task listener).
  const tRef = useRef(t);
  tRef.current = t;
  const sessionRef = useRef<string | null>(null);
  const permissions = useToolPermissions(currentSessionId, status === "busy");
  const agent = useAgentRun(lang);
  useEffect(() => {
    if (status === "busy" || agent.busy) return;
    const token = ++modelRequestRef.current;
    void resolveConversationModel(modelSelection).then(model => {
      if (token !== modelRequestRef.current) return;
      setResolvedModel(model); setModelNotice(null);
    }).catch(cause => {
      if (token !== modelRequestRef.current) return;
      setResolvedModel(null);
      setModelNotice(modelErrorCopy(lang, cause));
    });
  }, [modelSelection, settingsRevision, status, agent.busy, lang]);
  const {
    viewedId: agentHistoryId,
    archive: agentHistoryArchive,
    open: openHistory,
    openScoped: openScopedAgentHistory,
    leave: leaveAgentHistory,
  } = useAgentHistoryView(agent.projection.active?.sessionId ?? null);
  const canContinueAgentHistory = agentHistoryId !== null && agentHistoryArchive?.agentDetails?.status !== "active"
    && catalogEntry?.ownership === "agent" && !catalogEntry.archived && catalogEntry.availability === "available";
  const readOnlyHistory = catalogEntry !== null && !catalogEntry.capabilities.send && !canContinueAgentHistory;
  const refreshSelectedConversation = useCallback(async (key: string) => {
    const displayed = catalogEntryRef.current;
    const generation = viewGenerationRef.current;
    if (!displayed || displayed.key !== key) return;
    try {
      const entry = await catalogAccess(key);
      if (generation !== viewGenerationRef.current || catalogEntryRef.current?.key !== key) return;
      if (entry.key !== key || JSON.stringify(entry.identity) !== JSON.stringify(displayed.identity)) throw new Error("history identity changed");
      catalogEntryRef.current = entry;
      setCatalogEntry(entry);
    } catch (cause) {
      if (generation !== viewGenerationRef.current || catalogEntryRef.current?.key !== key) return;
      const unavailable: UnifiedHistoryRow = { ...displayed, availability: "unavailable", runtime: "unknown",
        capabilities: { ...displayed.capabilities, send: false, readOnlyReason: "native_unavailable" } };
      catalogEntryRef.current = unavailable;
      setCatalogEntry(unavailable);
      console.warn("selected conversation unavailable", cause instanceof Error ? cause.message : String(cause));
    }
  }, []);
  useEffect(() => {
    const key = catalogEntry?.key;
    if (!key) return;
    let disposed = false;
    const subscription = listen("history-catalog-changed", () => {
      if (!disposed) void refreshSelectedConversation(key);
    }).catch(cause => {
      console.warn("history listener unavailable", cause instanceof Error ? cause.message : String(cause));
      return () => undefined;
    });
    return () => {
      disposed = true;
      void subscription.then(stop => stop());
    };
  }, [catalogEntry?.key, refreshSelectedConversation]);
  const agentHistoryActiveRef = useRef(agentHistoryId !== null);
  useLayoutEffect(() => {
    agentHistoryActiveRef.current = agentHistoryId !== null;
  }, [agentHistoryId]);
  const [petActivity] = useState(() => createChatPetActivity(broadcastPetActivity));
  const personaRef = useRef<PersonaData | null>(null);
  const activePersonaIdRef = useRef(DEFAULT_PERSONA_ID);
  const personaLoadRef = useRef<Promise<void>>(Promise.resolve());
  const personaLoadSequenceRef = useRef(0);
  const settingsRef = useRef<Settings | null>(null);
  /** role by server messageID; used to skip user-message parts in SSE. */
  const rolesRef = useRef<Map<string, string>>(new Map());
  const messagePersonasRef = useRef(new Map<string, string>());
  const listRef = useRef<HTMLDivElement>(null);
  const followLatestRef = useRef(true);
  const lastScrollUserRef = useRef<string | undefined>(undefined);
  const dragDepthRef = useRef(0);
  /** mirror of `messages` for persisting history outside render. */
  const messagesRef = useRef<ChatMessage[]>([]);
  const fixedReplySequenceRef = useRef(0);
  const replyRequestRef = useRef<{ readonly requestId: string | null } | null>(null);
  const messageParentsRef = useRef(new Map<string, string>());
  const [streamText] = useState(() => createStreamTextBuffer((updates) => {
    setMessages((previous) => {
      const remaining = new Map(updates.map(update => [update.messageId, update.text]));
      let changed = false;
      const next = previous.map(message => {
        const text = remaining.get(message.id);
        if (text === undefined || !isTextChatMessage(message)) return message;
        remaining.delete(message.id);
        const activity = preserveWebSearchActivity(message.activity);
        if (message.text === text && message.activity === activity) return message;
        changed = true;
        return { ...message, text, activity };
      });
      for (const [id, text] of remaining) {
        changed = true;
        next.push({ id, role: "assistant", personaId: messagePersonasRef.current.get(id), text });
      }
      return changed ? next : previous;
    });
  }));


  const appendArtifactMessage = useCallback((artifact: LocalAttachmentArtifact) => {
    setMessages((prev) => [
      ...prev,
      {
        id: `artifact-${artifact.state.artifact.id}-${Date.now()}`,
        role: "artifact",
        text: "",
        artifactId: artifact.state.artifact.id,
      },
    ]);
  }, []);

  const reportAttachmentBackgroundError = useCallback((message: string) => {
    setAttachmentError(message);
  }, []);

  const chatAttachments = useChatAttachments({
    sessionId: currentSessionId ?? "",
    personaId: activePersonaId,
    onArtifact: appendArtifactMessage,
    onBackgroundError: reportAttachmentBackgroundError,
  });
  const cleanupAttachmentSession = chatAttachments.cleanupSession;
  const discardSentAttachmentSources = chatAttachments.discardSentSources;
  const resetAttachmentSession = chatAttachments.resetSession;
  const localResources = useLocalResources({
    scopeKey: `${currentSessionId ?? ""}:${agent.workspace ?? ""}:${agentHistoryId ?? ""}`,
    active: view === "chat" && status === "ready" && !!currentSessionId && !isSubmitting && !agent.busy && !historyLoading && !readOnlyHistory && !sessionOwnedByWorkbench,
    useLegacyFiles: !agent.workspace && !agentHistoryId,
    onLegacyFiles: files => stageAttachmentFiles(files),
    onError: message => reportAttachmentBackgroundError(localResourceError(resourceLanguage, message)),
  });
  const attachmentBusy = localResources.pending || chatAttachments.items.some((item) => item.kind === "staging");
  const attachmentBlocked = chatAttachments.items.some(
    (item) => item.kind === "failed" && item.phase === "staging",
  );

  const cancelReplyPresentation = () => {
    streamText.flush();
    streamText.reset();
    replyRequestRef.current = null;
  };

  const updateAssistantTool = (messageID: string, activity: ToolActivity) => {
    streamText.flush();
    streamText.invalidate(messageID);
    setMessages((prev) =>
      upsertAssistant(prev, messageID, messagePersonasRef.current.get(messageID), (message) => ({
        ...message,
        activity,
      })),
    );
  };

  const clearAssistantTool = (messageID: string) => {
    streamText.flush();
    streamText.invalidate(messageID);
    setMessages((prev) =>
      prev.flatMap((message) => {
        if (message.id !== messageID) return [message];
        const next = { ...message, activity: undefined };
        return next.role === "assistant" && !hasVisibleMessageContent(next) ? [] : [next];
      }),
    );
  };

  const completeReplyPresentation = () => {
    streamText.flush();
    setIsPersonaTyping(false);
    setStatus("ready");
    setMessages((prev) =>
      prev.map((message) => {
        if (!isTextChatMessage(message) || !message.activity) return message;
        const activity = settleToolActivity(message.activity);
        return activity === message.activity ? message : { ...message, activity };
      }),
    );
  };

  const waitForFixedReplyStart = async (sequence: number): Promise<boolean> => {
    await waitForDelay(FIXED_REPLY_TYPING_DELAY_MS);
    if (fixedReplySequenceRef.current !== sequence) return false;
    setIsPersonaTyping(true);
    petActivity.mood("thinking");
    await waitForDelay(FIXED_REPLY_TYPING_DELAY_MS);
    return fixedReplySequenceRef.current === sequence;
  };

  useEffect(() => {
    return () => {
      petActivity.cancel();
      fixedReplySequenceRef.current += 1;
      streamText.reset();
      replyRequestRef.current = null;
    };
  }, [petActivity, streamText]);

  const resetSession = useCallback(async (preserveComposer = false): Promise<boolean> => {
    streamText.flush();
    streamText.reset();
    const generation = ++viewGenerationRef.current;
    const previousSession = sessionRef.current;
    const carriedSelection = modelSelectionRef.current;
    await waitForServer();
    const session = await createSession("YUME chat");
    const registered = await invoke<UnifiedHistoryRow>("history_register_native_session", {
      sessionId: session.id,
      directory: session.directory,
      source: "light_chat",
    });
    if (viewGenerationRef.current !== generation) return false;
    let ready = registered;
    let statusUnavailable = false;
    try {
      const fresh = await catalogAccess(registered.key);
      if (fresh.key !== registered.key || JSON.stringify(fresh.identity) !== JSON.stringify(registered.identity)) throw new Error("history identity changed");
      ready = fresh;
    } catch (error: unknown) {
      statusUnavailable = true;
      console.warn("new conversation status unavailable", error instanceof Error ? error.message : String(error));
    }
    if (viewGenerationRef.current !== generation) return false;
    if (preserveComposer && carriedSelection.mode === "override") {
      await setConversationModel(ready.key, carriedSelection);
    }
    if (previousSession && catalogEntryRef.current?.capabilities.send) {
      await abortSession(previousSession, sessionDirectory()).catch(() => undefined);
      await cleanupAttachmentSession(previousSession);
    }
    petActivity.cancel();
    fixedReplySequenceRef.current += 1;
    cancelReplyPresentation();
    setIsPersonaTyping(false);
    const nextSelection = preserveComposer ? carriedSelection : INHERIT_MODEL;
    modelSelectionRef.current = nextSelection;
    setModelSelection(nextSelection);
    setResolvedModel(null);
    setSettingsRevision(value => value + 1);
    catalogEntryRef.current = ready;
    setCatalogEntry(ready);
    void invoke("history_chat_selected", { key: ready.key }).catch(() => {});
    setMemoryNotice(statusUnavailable ? tRef.current.sessionStateUnknown : null);
    localReplySnapshotRef.current = false;
    setHistoryLoading(false);
    sessionRef.current = session.id;
    setCurrentSessionId(session.id);
    resetAttachmentSession(session.id);
    rolesRef.current.clear();
    messageParentsRef.current.clear();
    messagePersonasRef.current.clear();
    setMessages([]);
    if (!preserveComposer) setInput("");
    setView("chat");
    setStatus("ready");
    return true;
  }, [cleanupAttachmentSession, resetAttachmentSession, petActivity, streamText]);

  const loadPersona = useCallback((id: string): Promise<void> => {
    const resolvedId = resolvePersonaId(id);
    const sequence = ++personaLoadSequenceRef.current;
    const request = invoke<PersonaData>("load_persona", { id: resolvedId })
      .then((data) => {
        if (sequence !== personaLoadSequenceRef.current) return;
        personaRef.current = data;
        activePersonaIdRef.current = resolvedId;
        setActivePersonaId(resolvedId);
      })
      .catch((error: unknown) => {
        console.error(
          "persona load failed",
          error instanceof Error ? error : new Error(String(error)),
        );
      });
    personaLoadRef.current = request;
    return request;
  }, []);

  // Boot: wait for sidecar, load persona, create session, subscribe SSE.
  useEffect(() => {
    let closed = false;
    let stopSettingsListener: (() => void) | null = null;

    (async () => {
      try {
        settingsRef.current = await getSettings().catch(() => null);
        if (closed) return;
        const initialPersonaId = resolvePersonaId(
          settingsRef.current?.personaId,
        );
        if (settingsRef.current) {
          setLang(settingsRef.current.language);
          setTheme(settingsRef.current.theme);
        }
        await loadPersona(initialPersonaId);
        const settingsListener = await onSettingsChanged((s) => {
          settingsRef.current = s;
          setSettingsRevision(value => value + 1);
          setLang(s.language);
          setTheme(s.theme);
          const nextPersonaId = resolvePersonaId(s.personaId);
          if (
            shouldResetSessionForPersona(
              activePersonaIdRef.current,
              nextPersonaId,
            )
          ) {
            const switchRequest = loadPersona(nextPersonaId).then(async () => {
              if (activePersonaIdRef.current === nextPersonaId) await resetSession();
            });
            personaLoadRef.current = switchRequest;
            void switchRequest.catch((error: unknown) => {
              console.error(
                "persona session reset failed",
                error instanceof Error ? error : new Error(String(error)),
              );
            });
          }
        });
        // Unmounting while the listener was still being registered must still
        // detach it, otherwise it outlives the component.
        if (closed) {
          settingsListener();
          return;
        }
        stopSettingsListener = settingsListener;
        await resetSession();
        if (closed) return;
        setStatus("ready");
      } catch (error: unknown) {
        if (closed) return;
        console.error(
          error instanceof Error ? error : new Error(String(error)),
        );
        setStatus("error");
      }
    })();

    return () => {
      closed = true;
      personaLoadSequenceRef.current += 1;
      stopSettingsListener?.();
    };
  }, [loadPersona, resetSession]);

  const handleEvent = useCallback((e: OpenCodeEvent) => {
    if (agentHistoryActiveRef.current) return;
    const props = e.properties ?? {};

    switch (e.type) {
      case "message.updated": {
        const info = props.info as {
          id: string;
          sessionID: string;
          role: string;
          parentID?: string;
          time?: { completed?: number };
        };
        if (info.sessionID === sessionRef.current) {
          rolesRef.current.set(info.id, info.role);
          if (info.parentID) messageParentsRef.current.set(info.id, info.parentID);
          const personaId = info.parentID ? messagePersonasRef.current.get(info.parentID) : undefined;
          if (info.role === "assistant" && personaId) messagePersonasRef.current.set(info.id, personaId);
          petActivity.message(props.info);
          void recordCompletedAiUsage(props.info).catch(() => {});
          if (info.role === "assistant" && info.time?.completed !== undefined) streamText.flush();
        }
        break;
      }
      case "message.part.updated": {
        const part = props.part as {
          sessionID: string;
          messageID: string;
          id?: string;
          type: string;
          text?: string;
          tool?: string;
          state?: { title?: string; status?: string };
        };
        if (part.sessionID !== sessionRef.current || replyRequestRef.current === null) return;
        const parentID = messageParentsRef.current.get(part.messageID);
        if (parentID && replyRequestRef.current.requestId !== null && parentID !== replyRequestRef.current.requestId) return;
        petActivity.part(props.part);
        // Parts of the user's own message echo back over SSE; skip them.
        if (rolesRef.current.get(part.messageID) === "user") return;

        if (part.type === "text") {
          setIsPersonaTyping(true);
          streamText.push(part.messageID, part.id, part.text ?? "");
        } else if (part.type === "tool") {
          if (WORKLOG_TOOLS.some((tool) => tool === part.tool)) {
            worklog.acceptTool(part);
            clearAssistantTool(part.messageID);
            return;
          }
          const toolPart = toOpenCodeToolPart(part);
          if (!toolPart) {
            updateAssistantTool(part.messageID, part.state?.title || part.tool || "tool");
            return;
          }
          if (toolPart.tool === "websearch") {
            const running =
              toolPart.state.status === "pending" || toolPart.state.status === "running";
            updateAssistantTool(toolPart.messageID, {
              kind: "websearch",
              running,
              sites: webSearchSites(toolPart),
            });
            return;
          }
          updateAssistantTool(toolPart.messageID, toolPart.state.title ?? toolPart.tool);
        }
        break;
      }
      case "session.status": {
        if (props.sessionID !== sessionRef.current || replyRequestRef.current === null) return;
        const s = (props.status as { type: string } | undefined)?.type;
        if (s === "busy") {
          setStatus("busy");
        }
        break;
      }
      case "session.idle": {
        if (props.sessionID !== sessionRef.current || replyRequestRef.current === null) return;
        petActivity.idle(props.sessionID);
        if (typeof props.sessionID === "string") {
          void worklog.recover(props.sessionID);
        }
        completeReplyPresentation();
        break;
      }
      case "session.error": {
        if (replyRequestRef.current === null || (props.sessionID && props.sessionID !== sessionRef.current)) return;
        petActivity.sessionError(props.sessionID);
        cancelReplyPresentation();
        setIsPersonaTyping(false);
        setStatus("ready");
        const err = props.error as
          { name?: string; data?: { message?: string } } | undefined;
        const detail = err?.data?.message || err?.name || "unknown error";
        setMessages((prev) => [
          ...prev,
          {
            id: `err-${Date.now()}`,
            role: "assistant",
            text: `${personaRef.current?.placeholders?.error ?? tRef.current.chatErrorPrefix}: ${detail}`,
          },
        ]);
        break;
      }
    }
  }, []);

  useEffect(() => {
    if (!currentSessionId || !currentDirectory) return;
    let closed = false;
    const subscription = subscribeEvents((event) => {
      const identity = catalogEntryRef.current?.identity;
      if (!closed && identity?.kind === "native" && identity.directory === currentDirectory && identity.sessionId === currentSessionId) handleEvent(event);
    }, currentDirectory);
    return () => {
      closed = true;
      void subscription.then((stop) => stop());
    };
  }, [currentSessionId, currentDirectory, handleEvent]);

  useEffect(() => {
    const lastUser = messages.filter((message) => message.role === "user").at(-1)?.id;
    if (lastUser !== lastScrollUserRef.current) followLatestRef.current = true;
    lastScrollUserRef.current = lastUser;
    if (followLatestRef.current) listRef.current?.scrollTo({ top: listRef.current.scrollHeight });
  }, [messages, view]);

  // Mirror messages into a ref for history persistence.
  useEffect(() => {
    messagesRef.current = messages;
  }, [messages]);

  // Persist only YUME-local messages; native text remains owned by OpenCode.
  useEffect(() => {
    if (agentHistoryId || !localReplySnapshotRef.current || status !== "ready" || messages.length === 0) return;
    const sessionID = sessionRef.current;
    if (!sessionID) return;
    const timer = setTimeout(() => {
      if (agentHistoryActiveRef.current || !localReplySnapshotRef.current || sessionRef.current !== sessionID) return;
      const msgs = messagesRef.current;
      const localMessages = msgs.flatMap(message => isTextChatMessage(message) && message.localOnly && message.text.trim()
        ? [{ role: message.role, text: message.text, time: message.time ?? Date.now(), messageId: message.id, localOnly: true, personaId: message.personaId }]
        : []);
      const entry = catalogEntryRef.current;
      if (!entry || !localMessages.length) return;
      void saveLocalMessages(entry.key, localMessages)
        .catch(error => console.error("local reply save failed", error instanceof Error ? error.message : String(error)));
    }, 300);
    return () => clearTimeout(timer);
  }, [agentHistoryId, messages, status]);

  useEffect(() => {
    if (agentHistoryArchive) setMessages(historyChatMessages(agentHistoryArchive));
  }, [agentHistoryArchive]);

  const sendCurrentDraft = async () => {
    const generation = viewGenerationRef.current;
    const roundSelection = modelSelectionRef.current;
    const text = input.trim();
    const roundResources = localResources.resources;
    if (!text && chatAttachments.items.length === 0 && roundResources.length === 0) return;
    if (historyLoading || readOnlyHistory || sessionOwnedByWorkbench || status !== "ready" || !sessionRef.current) return;
    if (attachmentBusy) { setAttachmentError(t.chatAttachmentStillReading); return; }
    if (containsProviderApiKey(text)) {
      setInput("");
      setAttachmentError(null);
      setMemoryNotice(t.providerSecretRedirect);
      return;
    }
    if (agent.workspace || agentHistoryId || agent.busy) {
      if ((!text && roundResources.length === 0) || agent.busy) return;
      if (chatAttachments.items.length > 0) {
        setAttachmentError(t.agentAttachmentsUnsupported);
        return;
      }
      let roundModel: ChatModelChoice;
      try {
        roundModel = await resolveConversationModel(roundSelection);
      } catch (cause) {
        if (generation === viewGenerationRef.current) setModelNotice(modelErrorCopy(lang, cause));
        return;
      }
      if (generation !== viewGenerationRef.current || roundSelection !== modelSelectionRef.current) return;
      setResolvedModel(roundModel);
      if (agentHistoryId && catalogEntryRef.current?.identity.kind === "native") {
        const displayed = catalogEntryRef.current;
        try {
          const loaded = await loadSharedConversation(displayed);
          if (generation !== viewGenerationRef.current) return;
          catalogEntryRef.current = loaded.entry;
          setCatalogEntry(loaded.entry);
          if (loaded.entry.archived || loaded.entry.tombstone || loaded.entry.availability !== "available"
            || loaded.entry.runtime === "unknown" || loaded.entry.runtime === "running" || loaded.agentDetails?.status === "active") return;
        } catch (cause) {
          await refreshSelectedConversation(displayed.key);
          if (generation !== viewGenerationRef.current) return;
          setMemoryNotice(tRef.current.sessionStateUnknown);
          console.warn("agent conversation access unavailable", cause instanceof Error ? cause.message : String(cause));
          return;
        }
      }
      if (generation !== viewGenerationRef.current || roundSelection !== modelSelectionRef.current) return;
      const run = await agent.start(text || `附件：${roundResources.map(resource => resource.name).join(", ")}`, agentHistoryId ?? undefined,
        agentHistoryId && catalogEntryRef.current?.identity.kind === "native" ? catalogEntryRef.current.key : undefined,
        roundSelection, roundModel, roundResources.map(resource => resource.id));
      if (generation !== viewGenerationRef.current) return;
      if (run) {
        localResources.clearSent(roundResources.map(resource => resource.id));
        setInput("");
        setAttachmentError(null);
        if (run.sessionId) {
          try {
            const key = await invoke<string>("history_catalog_native_key", { directory: run.workspacePath, sessionId: run.sessionId });
            const loaded = await catalogLoad(key);
            if (generation !== viewGenerationRef.current) return;
            openScopedAgentHistory(loaded);
            catalogEntryRef.current = loaded.entry;
            setCatalogEntry(loaded.entry);
            sessionRef.current = run.sessionId;
            setCurrentSessionId(run.sessionId);
            setMessages(historyChatMessages({
              id: run.sessionId, title: loaded.entry.displayTitle, created: loaded.entry.created,
              updated: loaded.entry.updated, messages: loaded.messages, originRunId: run.runId,
              agentDetails: loaded.agentDetails,
            } satisfies HistorySession));
          } catch (cause) {
            setMemoryNotice(tRef.current.historyLoadFailed);
            console.warn("agent catalog adoption failed", cause instanceof Error ? cause.message : String(cause));
          }
        }
      }
      return;
    }
    if (attachmentBusy) {
      setAttachmentError(t.chatAttachmentStillReading);
      return;
    }
    if (attachmentBlocked) {
      setAttachmentError(t.chatAttachmentFixErrors);
      return;
    }
    let prepared: PreparedModelAttachments;
    try {
      prepared = await chatAttachments.prepareModelAttachments(text);
    } catch (error: unknown) {
      if (generation !== viewGenerationRef.current || (error instanceof DOMException && error.name === "AbortError")) return;
      setAttachmentError(error instanceof Error ? error.message : t.chatAttachmentReadFailed);
      return;
    }
    if ((!prepared.shouldSendToModel && roundResources.length === 0) || generation !== viewGenerationRef.current) return;
    const sentReadyLocalIds = chatAttachments.items
      .filter((item) => item.kind === "ready")
      .map((item) => item.localId);
    setInput("");
    setAttachmentError(null);
    const sent = await sendText(text, prepared, roundSelection, roundResources);
    if (!sent && generation === viewGenerationRef.current) setInput(text);
    if (sent) {
      discardSentAttachmentSources(sentReadyLocalIds);
      if (generation === viewGenerationRef.current) localResources.clearSent(roundResources.map(resource => resource.id));
    }
  };

  const send = async () => {
    // Resource preparation crosses IPC before status becomes busy. Lock synchronously
    // so repeated Enter/click events cannot submit or discard the same draft twice.
    if (submittingRef.current) return;
    submittingRef.current = true;
    setIsSubmitting(true);
    try { await sendCurrentDraft(); }
    finally { submittingRef.current = false; setIsSubmitting(false); }
  };

  const sendText = async (
    text: string,
    prepared: PreparedModelAttachments = EMPTY_PREPARED_ATTACHMENTS,
    roundSelection: ConversationModelSelection = modelSelectionRef.current,
    resources: readonly LocalResource[] = [],
  ): Promise<boolean> => {
    const sessionID = sessionRef.current;
    const generation = viewGenerationRef.current;
    const displayed = catalogEntryRef.current;
    if (!displayed || historyLoading || sessionOwnedByWorkbench || !displayed.capabilities.send) return false;
    if ((!text && !prepared.shouldSendToModel && resources.length === 0) || !sessionID) return false;
    await personaLoadRef.current;
    if (sessionRef.current !== sessionID || generation !== viewGenerationRef.current) return false;
    let fresh: UnifiedHistoryRow;
    try {
      fresh = await catalogAccess(displayed.key);
    } catch (error: unknown) {
      setMemoryNotice(tRef.current.sessionStateUnknown);
      console.error("conversation access unavailable", error instanceof Error ? error.message : String(error));
      return false;
    }
    if (generation !== viewGenerationRef.current) return false;
    catalogEntryRef.current = fresh;
    setCatalogEntry(fresh);
    const identity = validateSharedConversation(displayed, fresh);
    if (!identity) return false;
    let roundModel: ChatModelChoice;
    try {
      roundModel = await resolveConversationModel(roundSelection);
    } catch (cause) {
      if (generation === viewGenerationRef.current) setModelNotice(modelErrorCopy(lang, cause));
      return false;
    }
    if (generation !== viewGenerationRef.current || roundSelection !== modelSelectionRef.current) return false;
    setResolvedModel(roundModel);
    setModelNotice(null);
    const messageAttachments = prepared.fileParts.map(attachmentPreviewFromPart);
    const attachmentNames = [...messageAttachments, ...resources].map((item) => item.name).join(", ");
    let promptText = text || prepared.fallbackPrompt || (resources.length ? `附件：${attachmentNames}` : null);
    let resourceParts: readonly OpenCodeFilePart[] = [];
    const roundPersonaId = activePersonaIdRef.current;
    const localOnly = activePersonaIdRef.current === DEFAULT_PERSONA_ID && prepared.fileParts.length === 0 && resources.length === 0
      && (isXiaozhuNameOriginQuestion(text) || isXiaozhuIdentityQuestion(text));
    const userMessageId = localOnly ? `local_${newUserMessageId()}` : newUserMessageId();
    if (resources.length) {
      try {
        const references = await prepareChatResources(identity.directory, sessionID, userMessageId, resources.map(resource => resource.id));
        if (generation !== viewGenerationRef.current || sessionRef.current !== sessionID) return false;
        resourceParts = references.parts;
        promptText = [promptText, references.text].filter(Boolean).join("\n\n");
      } catch (cause) {
        if (generation === viewGenerationRef.current) setAttachmentError(localResourceError(resourceLanguage, cause));
        return false;
      }
    }
    cancelReplyPresentation();
    rolesRef.current.set(userMessageId, "user");
    messagePersonasRef.current.set(userMessageId, roundPersonaId);
    setMessages((prev) => [
      ...prev,
      {
        id: userMessageId,
        role: "user",
        personaId: roundPersonaId,
        text: text || `附件：${attachmentNames}`,
        attachments: messageAttachments,
        resources,
        localOnly, time: Date.now(),
      },
    ]);
    if (!promptText) return false;
    const petScope = { sessionId: sessionID, requestId: userMessageId };
    petActivity.start(petScope);
    setStatus("busy");
    setIsPersonaTyping(false);
    const fixedReplySequence = fixedReplySequenceRef.current + 1;
    fixedReplySequenceRef.current = fixedReplySequence;
    if (
      activePersonaIdRef.current === DEFAULT_PERSONA_ID &&
      prepared.fileParts.length === 0 && resources.length === 0
    ) {
      if (isXiaozhuNameOriginQuestion(text)) {
        localReplySnapshotRef.current = true;
        if (!(await waitForFixedReplyStart(fixedReplySequence))) return false;
        for (const [index, line] of XIAOZHU_NAME_ORIGIN_LINES.entries()) {
          if (index > 0) {
            setIsPersonaTyping(true);
            petActivity.mood("thinking");
            await new Promise<void>((resolve) => {
              globalThis.setTimeout(resolve, 2000);
            });
            if (fixedReplySequenceRef.current !== fixedReplySequence) return false;
          }
          setIsPersonaTyping(false);
          setMessages((prev) => [
            ...prev,
            {
              id: `${userMessageId}_origin_${index}`,
              role: "assistant",
              personaId: roundPersonaId,
              text: line, localOnly: true, time: Date.now(),
            },
          ]);
          petActivity.mood("talking");
        }
        setIsPersonaTyping(false);
        setStatus("ready");
        petActivity.success(petScope);
        return true;
      }
      if (isXiaozhuIdentityQuestion(text)) {
        localReplySnapshotRef.current = true;
        if (!(await waitForFixedReplyStart(fixedReplySequence))) return false;
        setIsPersonaTyping(false);
        setMessages((prev) => [
          ...prev,
          {
            id: `${userMessageId}_identity`,
            role: "assistant",
            personaId: roundPersonaId,
            text: XIAOZHU_IDENTITY_REPLY, localOnly: true, time: Date.now(),
          },
        ]);
        setStatus("ready");
        petActivity.success(petScope);
        return true;
      }
    }
    replyRequestRef.current = { requestId: userMessageId };
    const matchesSubmission = () => generation === viewGenerationRef.current
      && sessionID === sessionRef.current && replyRequestRef.current?.requestId === userMessageId;
    const isCurrentSubmission = () => matchesSubmission() && petActivity.isCurrent(petScope);
    let promptStarted = false;
    try {
      const s = settingsRef.current;
      // Persona defaults to Chinese; add a reply-language override otherwise.
      const langNames: Record<string, string> = {
        "en-US": "English",
        "ja-JP": "日本語",
        "ko-KR": "한국어",
      };
      const langName = s ? langNames[s.language] : undefined;
      const persona = personaRef.current?.persona;
      const personaPrompt = persona
        ? langName
          ? `${persona}${personaRef.current?.skills ? `\n\n${personaRef.current.skills}` : ""}\n\n# 回复语言\n\n- 使用 ${langName} 回复(用户界面语言已切换)`
          : `${persona}${personaRef.current?.skills ? `\n\n${personaRef.current.skills}` : ""}`
        : undefined;
      // Relevant confirmed memories, appended after the persona so they can
      // only add context to the instructions above them. A memory failure
      // yields an empty block rather than blocking the turn.
      const memoryBlock = await memoryBlockForTurn({
        personaId: activePersonaIdRef.current,
        userText: promptText,
        enabled: s?.memoryAiUse ?? true,
        directory: identity.directory,
      });
      const system = composeSystemPrompt({
        personaPrompt,
        memoryBlock,
        userNameInstruction: userNameInstruction(s?.userName ?? ""),
      });
      try {
        await registerWorklogTurn(sessionID, userMessageId, text, identity.directory);
      } catch (error: unknown) {
        setMemoryNotice(`${worklogChatCopy(lang).failed}: ${error instanceof Error ? error.message : "BRIDGE_UNAVAILABLE"}`);
      }
      if (!petActivity.isCurrent(petScope) || generation !== viewGenerationRef.current) return false;
      const finalAccess = await catalogAccess(displayed.key);
      if (generation !== viewGenerationRef.current || !validateSharedConversation(displayed, finalAccess)) {
        if (generation === viewGenerationRef.current) {
          catalogEntryRef.current = finalAccess;
          setCatalogEntry(finalAccess);
          cancelReplyPresentation();
          setStatus("ready");
        }
        return false;
      }
      // Persist identity before submission; the host validates native completion.
      await invoke("memory_register_turn", { registration: {
        directory: identity.directory, sessionId: sessionID, messageId: userMessageId,
        providerId: roundModel.sidecarId, modelId: roundModel.modelId,
      } }).catch(() => { /* The managed plugin also registers the native turn. */ });
      if (!isCurrentSubmission()) return false;
      promptStarted = true;
      await promptAsync(sessionID, promptText, {
        directory: identity.directory,
        messageID: userMessageId,
        system: [personaSystemMarker(roundPersonaId), system, buildCurrentInformationInstruction(), buildWorklogSystemInstruction()].filter(Boolean).join("\n\n"),
        attachments: [...prepared.fileParts, ...resourceParts],
        model: { providerID: roundModel.sidecarId, modelID: roundModel.modelId },
      });
      return true;
    } catch (error: unknown) {
      if (!matchesSubmission()) return false;
      if (promptStarted) {
        const submission = await confirmPromptSubmission(sessionID, userMessageId, identity.directory);
        if (!matchesSubmission()) return false;
        if (submission === "confirmed") return true;
        // Stop/switch clear the request identity; an unchanged identity can already have
        // finished through native SSE while this read was pending. Never restore that draft.
        if (!petActivity.isCurrent(petScope)) return true;
        if (submission === "unknown") {
          setMemoryNotice(tRef.current.chatSubmissionUnknown);
          petActivity.mood("thinking");
          return true;
        }
      }
      petActivity.error(petScope);
      console.error(
        error instanceof Error ? error : new Error(String(error)),
      );
      cancelReplyPresentation();
      setIsPersonaTyping(false);
      setStatus("ready");
      // Only the optimistic row is ours to retract; native/unknown submissions stay intact.
      setMessages(previous => previous.filter(message => message.id !== userMessageId));
      setAttachmentError(prepared.fileParts.length > 0 || resources.length > 0
        ? tRef.current.chatAttachmentSendFailed : tRef.current.chatSendFailed);
      return false;
    }
  };

  const stageAttachmentFiles = useCallback((files: ArrayLike<File> | null) => {
    if (agent.workspace || agentHistoryId) {
      setAttachmentError(tRef.current.agentAttachmentsUnsupported);
      return;
    }
    if (catalogEntryRef.current && !catalogEntryRef.current.capabilities.send) return;
    const selected = Array.from(files ?? []);
    if (selected.length === 0) return;
    void (async () => {
      let sessionID = sessionRef.current;
      if (!sessionID) {
        await resetSession();
        sessionID = sessionRef.current;
      }
      if (!sessionID) {
        setAttachmentError(tRef.current.chatAttachmentStillReading);
        return;
      }
      resetAttachmentSession(sessionID);
      setCurrentSessionId(sessionID);
      chatAttachments.stageFromPicker(selected);
    })().catch((error: unknown) => {
      setAttachmentError(error instanceof Error ? error.message : tRef.current.chatAttachmentReadFailed);
    });
  }, [agent.workspace, agentHistoryId, chatAttachments, resetAttachmentSession, resetSession]);

  const pickAttachmentFiles = async () => {
    if (submittingRef.current || attachmentBusy || status !== "ready" || !currentSessionId || agent.busy) return;
    setOpenPicker(null);
    setAttachmentError(null);
    await localResources.pick(t.chatAttach);
  };

  const stageInputFiles = (files: ArrayLike<File>) => {
    const selected = Array.from(files);
    const media = selected.filter(isMediaFile);
    const legacy = selected.filter(file => !isMediaFile(file));
    if (media.length) void localResources.addMediaFiles(media);
    if (legacy.length) stageAttachmentFiles(legacy);
  };

  const handlePaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    if (event.clipboardData.files.length === 0) return;
    event.preventDefault();
    stageInputFiles(event.clipboardData.files);
  };

  const hasFiles = (event: DragEvent<HTMLDivElement>): boolean =>
    Array.from(event.dataTransfer.types).includes("Files");

  const handleDragEnter = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    dragDepthRef.current += 1;
    setIsDragActive(true);
  };

  const handleDragOver = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
    setIsDragActive(true);
  };

  const handleDragLeave = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFiles(event) && dragDepthRef.current === 0) return;
    event.preventDefault();
    dragDepthRef.current = Math.max(0, dragDepthRef.current - 1);
    if (dragDepthRef.current === 0) setIsDragActive(false);
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    dragDepthRef.current = 0;
    setIsDragActive(false);
    if (event.dataTransfer.files.length === 0) {
      setAttachmentError(t.chatAttachmentDropFailed);
      return;
    }
    stageInputFiles(event.dataTransfer.files);
  };

  // Bundled personas/skills failed to unpack: tell the user why instead of
  // letting it look like an empty persona list.
  useEffect(() => {
    const unlisten = onResourceError((reason) => {
      console.error("resource sync failed", reason);
      setAttachmentError(tRef.current.resourceSyncFailed);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const abort = async () => {
    const sessionID = sessionRef.current;
    if (!sessionID || isCancelling || sessionOwnedByWorkbench || readOnlyHistory || historyLoading) return;
    const generation = viewGenerationRef.current;
    const request = replyRequestRef.current;
    setIsCancelling(true);
    setMemoryNotice(null);
    try {
      await abortSession(sessionID, sessionDirectory());
      if (sessionRef.current !== sessionID || generation !== viewGenerationRef.current || request !== replyRequestRef.current) return;
      petActivity.cancel();
      fixedReplySequenceRef.current += 1;
      cancelReplyPresentation();
      setIsPersonaTyping(false);
      setStatus("ready");
    } catch (error: unknown) {
      if (sessionRef.current !== sessionID || generation !== viewGenerationRef.current || request !== replyRequestRef.current) return;
      const unknown = error instanceof SessionAbortError && error.code === "UNKNOWN";
      setMemoryNotice(unknown ? t.chatStopUnknown : t.chatStopFailed);
    } finally {
      setIsCancelling(false);
    }
  };

  /** Load actual native messages via the host, preserving the composite identity. */
  const resumeSession = useCallback(async (row: UnifiedHistoryRow) => {
    streamText.flush();
    streamText.reset();
    const generation = ++viewGenerationRef.current;
    setHistoryLoading(true);
    petActivity.cancel();
    fixedReplySequenceRef.current += 1;
    cancelReplyPresentation();
    setIsPersonaTyping(false);
    try {
      const [loaded, restoredSelection] = await Promise.all([loadSharedConversation(row), getConversationModel(row.key)]);
      if (generation !== viewGenerationRef.current) return;
      const entry = loaded.entry;
      if (entry.key !== row.key) throw new Error("history identity changed");
      if (generation !== viewGenerationRef.current) return;
      modelSelectionRef.current = restoredSelection;
      setModelSelection(restoredSelection);
      setResolvedModel(null);
      if (loaded.agentDetails) {
        const agentId = entry.identity.kind === "native" ? entry.identity.sessionId : entry.identity.historyId;
        const opened = entry.identity.kind === "native" ? openScopedAgentHistory(loaded) : await openHistory(agentId);
        if (generation !== viewGenerationRef.current || opened.kind !== "agent") return;
        agent.clearSelection();
        catalogEntryRef.current = entry;
        setCatalogEntry(entry);
        sessionRef.current = agentId;
        void invoke("history_chat_selected", { key: entry.key }).catch(() => {});
        setCurrentSessionId(agentId);
        setView("chat");
        setStatus("ready");
        return;
      }
      agent.clearSelection();
      leaveAgentHistory();
      const id = entry.identity.kind === "native" ? entry.identity.sessionId : null;
      const previousSession = sessionRef.current;
      if (previousSession && previousSession !== id) await cleanupAttachmentSession(previousSession);
      if (generation !== viewGenerationRef.current) return;
      sessionRef.current = id;
      // Opening native history observes that selected session; it does not confirm its run was cancelled.
      replyRequestRef.current = id ? { requestId: null } : null;
      catalogEntryRef.current = entry;
      setCatalogEntry(entry);
      setCurrentSessionId(id);
      if (id) resetAttachmentSession(id);
      rolesRef.current.clear();
      messageParentsRef.current.clear();
      messagePersonasRef.current.clear();
      for (const message of loaded.messages) {
        if (message.messageId && message.personaId) messagePersonasRef.current.set(message.messageId, message.personaId);
      }
      streamText.restore(loaded.messages.filter(message => message.role === "assistant" && !message.localOnly));
      setMessages(historyChatMessages(loaded));
      void invoke("history_chat_selected", { key: entry.key }).catch(() => {});
      setInput("");
      setPendingWorkspace(null);
      setMemoryNotice(null);
      setView("chat");
      setStatus("ready");
    } catch (error: unknown) {
      if (generation !== viewGenerationRef.current) return;
      setMemoryNotice(tRef.current.historyLoadFailed);
      throw error instanceof Error ? error : new Error(String(error));
    } finally {
      if (generation === viewGenerationRef.current) setHistoryLoading(false);
    }
  }, [agent, cleanupAttachmentSession, leaveAgentHistory, openHistory, openScopedAgentHistory, resetAttachmentSession, petActivity, streamText]);

  /** Start a fresh session. */
  const newChat = useCallback(async () => {
    try {
      if (!(await resetSession())) return;
      leaveAgentHistory();
      agent.clearSelection();
    } catch (error: unknown) {
      console.error(
        error instanceof Error ? error : new Error(String(error)),
      );
      const unknown = error instanceof SessionAbortError && error.code === "UNKNOWN";
      setMemoryNotice(unknown ? tRef.current.chatStopUnknown : tRef.current.chatStopFailed);
    }
  }, [agent, leaveAgentHistory, resetSession]);

  const selectModel = async (selection: ConversationModelSelection): Promise<void> => {
    if (status === "busy" || agent.busy || historyLoading || sessionOwnedByWorkbench || readOnlyHistory) throw new Error("model selection locked");
    const key = catalogEntryRef.current?.key;
    const generation = viewGenerationRef.current;
    if (!key) throw new Error("conversation unavailable");
    if (selection.mode === "override") await resolveConversationModel(selection);
    await setConversationModel(key, selection);
    if (generation !== viewGenerationRef.current || catalogEntryRef.current?.key !== key) return;
    modelSelectionRef.current = selection;
    setModelSelection(selection);
    setResolvedModel(null);
  };

  const folderLocked = status !== "ready" || agent.busy || agent.isStopping || historyLoading || sessionOwnedByWorkbench || readOnlyHistory;
  folderLockedRef.current = folderLocked;
  const commitFolder = async (path: string, expectedGeneration = viewGenerationRef.current): Promise<void> => {
    if (folderLockedRef.current) throw new Error(composerCopy(lang).folderLocked);
    const canonical = await invoke<string>("history_validate_workspace", { directory: path });
    if (expectedGeneration !== viewGenerationRef.current || folderLockedRef.current) throw new Error(composerCopy(lang).folderLocked);
    const current = agent.workspace ?? agentHistoryArchive?.agentDetails?.workspacePath ?? null;
    if (current === canonical) { setPendingWorkspace(null); return; }
    if (messagesRef.current.length > 0 || agentHistoryId || chatAttachments.items.length > 0 || localResources.resources.length > 0) {
      if (!(await resetSession(true))) return;
      leaveAgentHistory();
      for (const item of chatAttachments.items) chatAttachments.remove(item.localId);
    }
    if (folderLockedRef.current) throw new Error(composerCopy(lang).folderLocked);
    agent.selectWorkspace(canonical);
    setPendingWorkspace(null);
    await invoke("history_remember_workspace", { directory: canonical }).catch(cause => {
      console.warn("recent workspace unavailable", cause instanceof Error ? cause.message : String(cause));
    });
  };

  const chooseFolder = async (path?: string): Promise<void> => {
    if (folderLocked) throw new Error(composerCopy(lang).folderLocked);
    const generation = viewGenerationRef.current;
    const selected = path ?? await openDirectory({ directory: true, multiple: false, title: t.agentPickerTitle });
    if (typeof selected !== "string") return;
    if (generation !== viewGenerationRef.current) return;
    if (chatAttachments.items.length > 0 || localResources.resources.length > 0) {
      setPendingWorkspace({ path: selected, generation });
      return;
    }
    await commitFolder(selected, generation);
  };

  const leaveFolder = async (): Promise<void> => {
    if (folderLocked) throw new Error(composerCopy(lang).folderLocked);
    if (messagesRef.current.length > 0 || agentHistoryId) {
      if (!(await resetSession(true))) return;
    }
    leaveAgentHistory();
    agent.clearSelection();
    setPendingWorkspace(null);
  };
  const returnToAgentTask = async (): Promise<void> => {
    const run = agent.projection.active;
    if (!run?.sessionId) return;
    try {
      const key = await invoke<string>("history_catalog_native_key", { directory: run.workspacePath, sessionId: run.sessionId });
      const loaded = await catalogLoad(key);
      await resumeSession(loaded.entry);
    } catch (cause) {
      setMemoryNotice(tRef.current.historyLoadFailed);
      console.warn("agent task return failed", cause instanceof Error ? cause.message : String(cause));
    }
  };

  const closeChat = async (): Promise<void> => {
    try {
      await invoke("hide_chat");
    } catch (error: unknown) {
      console.error(
        "chat close failed",
        error instanceof Error ? error : new Error(String(error)),
      );
    }
  };

  const activePersonaName = personaDisplayName(activePersonaId, lang);
  const ownAvatar = userAvatarSource(settingsRef.current?.userAvatar);
  const ownAvatarSources = useMemo(() => ownAvatar ? [ownAvatar] : EMPTY_AVATAR_SOURCES, [ownAvatar]);
  const operationsByMessage = useMemo(() => {
    const byMessage = new Map<string, ChatWorklogOperation[]>();
    for (const operation of worklog.operations) {
      const entries = byMessage.get(operation.messageId);
      if (entries) entries.push(operation);
      else byMessage.set(operation.messageId, [operation]);
    }
    return byMessage;
  }, [worklog.operations]);
  const messageIds = useMemo(() => new Set(messages.map(message => message.id)), [messages]);

  const chatBooting = personalizePersonaCopy(t.chatBooting, activePersonaName);
  const chatEmpty = personalizePersonaCopy(t.chatEmpty, activePersonaName);
  const statusLabel: Record<Status, string> = {
    booting: chatBooting,
    ready: t.chatReady,
    busy: personaRef.current?.placeholders?.streaming ?? t.chatThinking,
    error: t.chatError,
  };

  return (
    <div
      className={`chat-root${isDragActive ? " chat-drag-active" : ""}`}
      data-theme={theme}
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {isDragActive && (
        <div className="chat-dropzone" role="status" aria-live="polite">
          {t.chatDropHere}
        </div>
      )}
      {view !== "history" && <header className="chat-header" data-tauri-drag-region="">
        <span className="chat-title">{activePersonaName}</span>
        <span className={`chat-status chat-status-${status}`}>
          {statusLabel[status]}
        </span>
        <button
          className="chat-iconbtn"
          disabled={!currentSessionId}
          onClick={() => {
            if (currentSessionId) {
              void openInWorkbench(currentSessionId, currentDirectory);
            }
          }}
          aria-label={t.openInWorkbench}
          title={t.openInWorkbench}
        >
          <AppIcon name="widget" size={18} />
        </button>
        <button
          className="chat-iconbtn"
          onClick={() => setView("history")}
          aria-label={t.tabHistory}
          title={t.tabHistory}
        >
          <AppIcon name="history" size={18} />
        </button>
        <button
          className="chat-settings"
          onClick={() => void invoke("open_settings")}
          aria-label={t.chatSettings}
          title={t.chatSettings}
        >
          <AppIcon name="general" size={18} />
        </button>
        <button
          className="chat-close"
          onClick={() => void closeChat()}
          aria-label={t.close}
        >
          <AppIcon name="close" size={18} />
        </button>
      </header>}
      {view !== "history" && workbenchOpenError?.sessionId === currentSessionId && (
        <div className="chat-memory-notice" role="alert">
          {workbenchOpenError.reason === "missing" ? t.workbenchSessionMissing : t.workbenchOpenFailed}
        </div>
      )}

      {view === "history" ? (
        <HistoryOrganizer language={lang} onOpen={resumeSession} onClose={() => setView("chat")} onNewChat={newChat} onChanged={refreshSelectedConversation}
          onDeleted={async (row, forgetMemories) => {
            const id = row.identity.kind === "native" ? row.identity.sessionId : row.identity.historyId;
            await cleanupAttachmentSession(id);
            if (forgetMemories) await memoryForgetConversation(id, row.key);
          }} />
      ) : (
        <>
          <div className="chat-timeline">
          <div className="chat-list" ref={listRef} onScroll={(event) => {
            const list = event.currentTarget;
            followLatestRef.current = list.scrollHeight - list.clientHeight - list.scrollTop < 48;
          }}>
            {messages.length === 0 && (
              <div className="chat-empty">
                <ChatText text={chatEmpty} format="plain" />
              </div>
            )}
            {messages.map((m) => {
              if (m.role === "artifact") {
                const artifactState = chatAttachments.artifacts.find(
                  (item) => item.artifact.id === m.artifactId,
                );
                if (artifactState === undefined) return null;
                return (
                  <ArtifactCard
                    key={m.id}
                    t={t}
                    state={artifactState}
                    onDownload={chatAttachments.download}
                    onRetry={chatAttachments.retryDownload}
                  />
                );
              }
              return (
                <ChatMessageRow key={m.id} message={m}
                  avatarSources={m.role === "user" ? ownAvatarSources : (personaAvatars[m.personaId ?? ""] ?? EMPTY_AVATAR_SOURCES)}
                  t={t} lang={lang} resourceLanguage={resourceLanguage}
                  currentDirectory={currentDirectory} currentSessionId={currentSessionId}
                  streaming={status === "busy" && m.role === "assistant" && m.id === messages.at(-1)?.id}
                  operations={operationsByMessage.get(m.id) ?? EMPTY_OPERATIONS}
                  onUndo={worklog.undo} onRefresh={worklog.refresh} />
              );
            })}
            {isPersonaTyping && (
              <div className="chat-typing" role="status" aria-live="polite">
                {t.chatTyping}
              </div>
            )}
            {worklog.operations.filter((operation) => !messageIds.has(operation.messageId)).map((operation) => <WorklogReceipt key={operation.requestId} operation={operation} language={lang} onUndo={worklog.undo} onRefresh={worklog.refresh} />)}
            <WorkspaceTask language={lang} agent={agent} historyDetails={agentHistoryArchive?.agentDetails} compact sessionId={currentSessionId} directory={currentDirectory} />
          </div>

          <ChatNavigation messages={messages} listRef={listRef} lang={lang} onNavigate={() => { followLatestRef.current = false; }} />
          </div>
          {memoryNotice && (
            <div className="chat-memory-notice" role="status" aria-live="polite">
              {memoryNotice}
            </div>
          )}

          {agent.projection.active && !runMatchesConversation(agent.projection.active, catalogEntry) && (
            <div className="chat-memory-notice" role="status">{t.agentBusy}
              <button type="button" className="chat-memory-action" onClick={() => void returnToAgentTask()}>{composerCopy(lang).returnTask}</button>
            </div>
          )}
          {runMatchesConversation(agent.projection.active, catalogEntry) && agent.requests.length > 0 && (
            <div className="chat-agent-approval-dock"><ToolApprovalCards requests={agent.requests} error={false} onReply={agent.reply} t={t} /></div>
          )}
          {sessionOwnedByWorkbench || (readOnlyHistory && !runMatchesConversation(agent.projection.active, catalogEntry)) || historyLoading ? (
            <div className="chat-memory-notice" role="status" aria-live="polite">
              {historyLoading ? t.loading : readOnlyHistory
                ? (catalogEntry ? conversationReadOnlyReason(catalogEntry, lang) : t.sessionStateUnknown)
                : workbenchSessionStateUnknown ? t.sessionStateUnknown : t.sessionOwnedByWorkbench}
              <button type="button" className="chat-memory-action" onClick={() => void newChat()}>{t.historyNewSession}</button>
              <button
                type="button"
                className="chat-memory-action"
                disabled={!currentSessionId}
                onClick={() => {
                  if (currentSessionId) {
                    void openInWorkbench(currentSessionId, currentDirectory);
                  }
                }}
              >
                {t.openInWorkbench}
              </button>
            </div>
          ) : (
            <>
              <ToolApprovalCards requests={permissions.requests} error={permissions.error} onReply={permissions.reply} t={t} />
              <footer ref={composerAnchor} className="chat-input-row">
              <LocalResourceTray resources={localResources.resources} lang={resourceLanguage} onRemove={localResources.remove}
                disabled={isSubmitting || status === "busy" || agent.busy} />
              <AttachmentTray
                t={t}
                disabled={isSubmitting}
                items={chatAttachments.items}
                error={attachmentError}
                onConfirm={chatAttachments.confirm}
                onCancel={chatAttachments.cancel}
                onRemove={chatAttachments.remove}
                onRetry={chatAttachments.retry}
              />
            <div className="chat-input-wrap">
              <div className="chat-textarea-wrap">
                <textarea
                  className="chat-input"
                  value={input}
                  disabled={isSubmitting}
                  placeholder={t.chatInputPlaceholder}
                  rows={2}
                  onChange={(e) => {
                    setInput(e.target.value);
                    if (e.target.value.trim()) setAttachmentError(null);
                  }}
                  onPaste={handlePaste}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                      e.preventDefault();
                      void send();
                    }
                  }}
                />
              </div>
              <div className="chat-composer-tools">
                <button
                  className="chat-attach"
                  type="button"
                  onClick={() => void pickAttachmentFiles()}
                  disabled={isSubmitting || status !== "ready" || !currentSessionId || attachmentBusy || agent.busy}
                  aria-label={t.chatAttach}
                  title={t.chatAttach}
                >
                  <AppIcon name="add" size={16} />
                </button>
                <WorkspacePicker language={lang} theme={theme} anchor={composerAnchor} workspace={agent.workspace ?? agentHistoryArchive?.agentDetails?.workspacePath ?? null}
                  locked={isSubmitting || folderLocked} onSelect={chooseFolder} onLeave={leaveFolder}
                  open={openPicker === "folder"} onOpenChange={open => setOpenPicker(open ? "folder" : null)} />
                <div className="chat-composer-spacer" />
                <ModelPicker language={lang} theme={theme} anchor={composerAnchor} selection={modelSelection} resolved={resolvedModel}
                  locked={isSubmitting || status === "busy" || agent.busy || historyLoading || readOnlyHistory || sessionOwnedByWorkbench}
                  onSelect={selectModel} onManage={() => void invoke("open_ai_model_settings")}
                  open={openPicker === "model"} onOpenChange={open => setOpenPicker(open ? "model" : null)} />
                {status === "busy" || runMatchesConversation(agent.projection.active, catalogEntry) ? (
                  <button className="chat-send chat-abort" type="button"
                    onClick={() => status === "busy" ? void abort() : void agent.stop()}
                    disabled={isCancelling || agent.isStopping} aria-label={t.chatStop}>
                    {isCancelling || agent.isStopping ? t.chatStopping : t.chatStop}
                  </button>
                ) : (
                  <button className="chat-send" type="button" onClick={() => void send()}
                    disabled={isSubmitting || agent.busy || status !== "ready" || attachmentBusy || !resolvedModel ||
                      (!input.trim() && localResources.resources.length === 0 && (agent.workspace || agentHistoryId ? true : chatAttachments.items.length === 0))}>
                    {t.chatSend}
                  </button>
                )}
              </div>
            </div>
            {modelNotice && <p className="chat-composer-notice" role="alert">{modelNotice}</p>}
            {pendingWorkspace && <div className="chat-folder-conflict" role="alertdialog" aria-label={composerCopy(lang).attachmentConflict}>
              <span>{composerCopy(lang).attachmentConflict}</span>
              <button type="button" onClick={() => setPendingWorkspace(null)}>{composerCopy(lang).keepAttachments}</button>
              <button type="button" onClick={() => void commitFolder(pendingWorkspace.path, pendingWorkspace.generation).catch(cause => setAttachmentError(folderErrorCopy(lang, cause)))}>{composerCopy(lang).removeAttachments}</button>
            </div>}
          </footer>
        </>
      )}
      </>
    )}
    </div>
  );
}

function attachmentPreviewFromPart(part: OpenCodeFilePart): ModelReadyAttachment {
  switch (part.mime) {
    case "image/gif":
    case "image/jpeg":
    case "image/png":
    case "image/webp":
      return {
        id: part.filename,
        name: part.filename,
        mime: part.mime,
        size: 0,
        kind: "image",
        status: "ready",
        dataUrl: part.url,
      };
    default:
      return {
        id: part.filename,
        name: part.filename,
        mime: "text/plain",
        size: 0,
        kind: "text",
        status: "ready",
        dataUrl: part.url,
        truncated: false,
      };
  }
}

function isTextChatMessage(message: ChatMessage): message is TextChatMessage {
  return message.role === "user" || message.role === "assistant";
}

function hasVisibleMessageContent(message: ChatMessage): boolean {
  if (message.role === "artifact") return true;
  return (
    message.text.trim().length > 0 ||
    (message.attachments !== undefined && message.attachments.length > 0)
  );
}

/** Update the assistant message with the given id, creating it if missing. */
function upsertAssistant(
  prev: ChatMessage[],
  messageID: string,
  personaId: string | undefined,
  update: (m: TextChatMessage) => TextChatMessage,
): ChatMessage[] {
  const idx = prev.findIndex((m) => m.id === messageID);
  if (idx >= 0) {
    return prev.map((message, index) => (
      index === idx && isTextChatMessage(message) ? update(message) : message
    ));
  }
  return [
    ...prev,
    update({ id: messageID, role: "assistant", personaId, text: "" }),
  ];
}


