import { memo } from "react";
import type { dict, Lang } from "../lib/i18n";
import { AppIcon } from "../ui/AppIcon";
import { Avatar } from "../ui/Avatar";
import type { ModelReadyAttachment } from "./attachments";
import { ChatText } from "./ChatText";
import { stripLocalResourceContext, type LocalResource } from "./localResources";
import { MessageLocalResources } from "./MessageLocalResources";
import { historicalPersonaName } from "./messagePersona";
import type { ChatWorklogOperation } from "./useWorklogChat";
import { WorklogReceipt } from "./WorklogReceipt";

export interface TextChatMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
  activity?: ToolActivity;
  attachments?: ModelReadyAttachment[];
  resources?: readonly LocalResource[];
  readonly nativeMessageId?: string;
  readonly localOnly?: boolean;
  readonly time?: number;
  readonly personaId?: string;
}

type WebSearchActivity = {
  readonly kind: "websearch";
  readonly running: boolean;
  readonly sites: readonly string[];
};

export type ToolActivity = string | WebSearchActivity;

interface ChatMessageRowProps {
  readonly message: TextChatMessage;
  readonly avatarSources: readonly string[];
  readonly lang: string;
  readonly resourceLanguage: Lang;
  readonly t: ReturnType<typeof dict>;
  readonly currentDirectory: string | undefined;
  readonly currentSessionId: string | null;
  readonly streaming: boolean;
  readonly operations: readonly ChatWorklogOperation[];
  readonly onUndo: (operation: ChatWorklogOperation) => Promise<void>;
  readonly onRefresh: (operation: ChatWorklogOperation) => Promise<void>;
}

export const ChatMessageRow = memo(function ChatMessageRow({
  message: m, avatarSources, lang, resourceLanguage, t, currentDirectory,
  currentSessionId, streaming, operations, onUndo, onRefresh,
}: ChatMessageRowProps) {
  return (
    <div data-message-id={m.id} className={`chat-msg chat-msg-${m.role}`}>
      <Avatar sources={avatarSources}
        label={m.role === "user" ? t.myAvatar : historicalPersonaName(m.personaId, lang)}
        fallback={m.role === "assistant" && m.personaId ? historicalPersonaName(m.personaId, lang) : undefined} />
      <div className="chat-msg-content">
      {m.activity && (
        <div
          className={`chat-activity${typeof m.activity === "object" ? ` chat-activity-websearch${m.activity.running ? " chat-activity-running" : ""}` : ""}`}
          role={typeof m.activity === "object" ? "status" : undefined}
          aria-live={typeof m.activity === "object" ? "polite" : undefined}
        >
          {typeof m.activity === "object" ? (
            <>
              <AppIcon
                name="network"
                size={16}
                className="chat-activity-icon app-icon-network"
              />
              <span>{t.chatWebSearch}</span>
              {m.activity.sites.length > 0 && (
                <>
                  <span className="chat-websearch-divider" aria-hidden="true">
                    ·
                  </span>
                  <span
                    className={`chat-websearch-sites${m.activity.sites.length > 1 ? " chat-websearch-sites-scrolling" : ""}`}
                    title={m.activity.sites.join(" · ")}
                  >
                    <span className="chat-websearch-sites-track">
                      <span className="chat-websearch-sites-group">
                        {m.activity.sites.join(" · ")}
                      </span>
                      {m.activity.sites.length > 1 && (
                        <span
                          className="chat-websearch-sites-group"
                          aria-hidden="true"
                        >
                          {m.activity.sites.join(" · ")}
                        </span>
                      )}
                    </span>
                  </span>
                </>
              )}
            </>
          ) : (
            <>
              <AppIcon name="general" size={16} className="chat-activity-icon" />{" "}
              {m.activity}
            </>
          )}
        </div>
      )}
      {m.attachments && m.attachments.length > 0 && (
        <div className="chat-message-attachments">
          {m.attachments.map((attachment) => (
            <AttachmentPreview
              key={attachment.id}
              attachment={attachment}
            />
          ))}
        </div>
      )}
      {m.role === "user" && (m.resources?.length || m.text.includes("<yume-local-resources>")) ? (
        <MessageLocalResources directory={currentDirectory ?? ""} sessionId={currentSessionId ?? ""}
          messageId={m.nativeMessageId ?? m.id} resources={m.resources} lang={resourceLanguage} />
      ) : null}
      {m.localOnly && m.role === "assistant" && <div className="chat-activity">{lang === "zh-CN" ? "本地回复" : "Local reply"}</div>}
      {(m.text.trim().length > 0 || m.role === "user") && (
        <div className="chat-bubble">
          <ChatText text={m.role === "user" ? stripLocalResourceContext(m.text) : m.text} format={m.role === "assistant" ? "markdown" : "plain"}
            streaming={streaming}
            lang={lang} />
        </div>
      )}
      {operations.map((operation) => <WorklogReceipt key={operation.requestId} operation={operation} language={lang} onUndo={onUndo} onRefresh={onRefresh} />)}
      </div>
    </div>
  );
});

function AttachmentPreview({
  attachment,
}: {
  attachment: ModelReadyAttachment;
}) {
  if (attachment.kind === "image") {
    return (
      <img
        className="chat-attachment-image"
        src={attachment.dataUrl}
        alt={attachment.name}
        title={attachment.name}
      />
    );
  }
  return <div className="chat-attachment-document">文档 · {attachment.name}</div>;
}

