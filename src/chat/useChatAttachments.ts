import { useCallback, useEffect, useRef, useState } from "react";
import { cleanupChatSession, convertStagedNcm, discardChatAttachment, exportChatArtifact, readChatAttachment, stageChatAttachment } from "./attachmentApi";
import { beginStagingAttachment, reduceAttachmentState, type AttachmentLifecycleState } from "./attachmentState";
import { committedAttachmentBudget } from "./useChatAttachmentBudgets";
import { discardCurrentSource, errorMessage, findArtifact, findItem, ignoreArtifact, ignoreBackgroundError, makeFallbackId, reduceArtifact } from "./useChatAttachmentsState";
import type { ArtifactState, AttachmentFileEvent, RetryUpload, UseChatAttachmentsOptions, UseChatAttachmentsResult } from "./useChatAttachmentsTypes";
import { buildPreparedModelAttachments, inferDraft, planStageUpload, prepareOpenCodeFilePart, toGeneratedArtifact, toStagedSource } from "./useChatAttachmentsUtils";

export type { ArtifactState, AttachmentFileEvent, LocalAttachmentArtifact, UseChatAttachmentsOptions, UseChatAttachmentsResult } from "./useChatAttachmentsTypes";

export function useChatAttachments(options: UseChatAttachmentsOptions): UseChatAttachmentsResult {
  const host = options.host;
  const makeLocalId = options.makeLocalId ?? makeFallbackId;
  const onArtifact = options.onArtifact ?? ignoreArtifact;
  const onBackgroundError = options.onBackgroundError ?? ignoreBackgroundError;
  const sessionIdRef = useRef(options.sessionId);
  const mountedRef = useRef(true);
  const generationRef = useRef(0);
  const previewUrlsRef = useRef(new Map<string, string>());
  const itemsRef = useRef<readonly AttachmentLifecycleState[]>([]);
  const artifactsRef = useRef<readonly ArtifactState[]>([]);
  const retryUploadsRef = useRef(new Map<string, RetryUpload>());
  const fileReadersRef = useRef(new Map<string, () => ReturnType<typeof planStageUpload>>());
  const preparationRef = useRef<{ controller: AbortController; localIds: Set<string> } | null>(null);
  const [items, setItems] = useState<readonly AttachmentLifecycleState[]>([]);
  const [artifacts, setArtifacts] = useState<readonly ArtifactState[]>([]);

  const setComposerItems = useCallback((next: readonly AttachmentLifecycleState[]) => {
    itemsRef.current = next;
    setItems(next);
  }, []);

  const setArtifactItems = useCallback((next: readonly ArtifactState[]) => {
    artifactsRef.current = next;
    setArtifacts(next);
  }, []);

  const releasePreview = useCallback((localId: string) => {
    const url = previewUrlsRef.current.get(localId);
    if (!url) return;
    URL.revokeObjectURL(url);
    previewUrlsRef.current.delete(localId);
  }, []);

  const releaseAllPreviews = useCallback(() => {
    for (const localId of previewUrlsRef.current.keys()) releasePreview(localId);
  }, [releasePreview]);

  const discardBackend = useCallback((sessionId: string, attachmentId: string) => {
    discardChatAttachment({ sessionId, attachmentId }, host)
      .catch((error: unknown) => onBackgroundError(errorMessage(error)));
  }, [host, onBackgroundError]);

  const dispatchItem = useCallback((localId: string, event: Parameters<typeof reduceAttachmentState>[1]) => {
    setComposerItems(itemsRef.current.map((item) => (
      item.localId === localId ? reduceAttachmentState(item, event) : item
    )));
  }, [setComposerItems]);

  const runUpload = useCallback((localId: string, retry: RetryUpload, token: number, generation: number) => {
    const isCurrent = () => mountedRef.current && generationRef.current === generation
      && findItem(itemsRef.current, localId)?.kind === "staging"
      && findItem(itemsRef.current, localId)?.operationToken === token;
    if (!isCurrent()) return;
    if (retry.plan.kind === "reject") {
      dispatchItem(localId, { type: "stageFailed", operationToken: token, message: retry.plan.message });
      return;
    }
    const request = retry.plan.request;
    stageChatAttachment(request, host, { uploadId: crypto.randomUUID(), scopeKey: retry.sessionId })
      .then((staged) => {
        if (!isCurrent()) {
          discardBackend(retry.sessionId, staged.id);
          return;
        }
        let previewUrl: string | undefined;
        try {
          if (/^image\/(gif|jpeg|png|webp)$/.test(staged.mime)) {
            previewUrl = URL.createObjectURL(new Blob([request.bytes], { type: staged.mime }));
            previewUrlsRef.current.set(localId, previewUrl);
          }
        } catch (error: unknown) {
          discardBackend(retry.sessionId, staged.id);
          throw error;
        }
        retryUploadsRef.current.delete(localId);
        dispatchItem(localId, {
          type: "stageSucceeded", operationToken: token,
          source: toStagedSource(staged, retry.sourceKind, previewUrl),
        });
      })
      .catch((error: unknown) => {
        if (isCurrent()) dispatchItem(localId, { type: "stageFailed", operationToken: token, message: errorMessage(error) });
      });
  }, [discardBackend, dispatchItem, host]);

  const readAndUpload = useCallback((localId: string, sourceKind: RetryUpload["sourceKind"], sessionId: string, token: number, generation: number) => {
    const read = fileReadersRef.current.get(localId);
    if (!read) return;
    read().then((plan) => {
      const current = findItem(itemsRef.current, localId);
      if (!mountedRef.current || generationRef.current !== generation
        || current?.kind !== "staging" || current.operationToken !== token) return;
      fileReadersRef.current.delete(localId);
      const retry = { sessionId, sourceKind, plan } satisfies RetryUpload;
      retryUploadsRef.current.set(localId, retry);
      runUpload(localId, retry, token, generation);
    }).catch((error: unknown) => {
      if (!mountedRef.current || generationRef.current !== generation) return;
      dispatchItem(localId, { type: "stageFailed", operationToken: token, message: errorMessage(error) });
    });
  }, [dispatchItem, runUpload]);

  const stageFiles = useCallback((files: ArrayLike<File> | null) => {
    if (!mountedRef.current) return;
    const generation = generationRef.current;
    for (const file of Array.from(files ?? [])) {
      const localId = makeLocalId();
      const draft = inferDraft(file, localId);
      const staging = beginStagingAttachment(draft);
      const sessionId = sessionIdRef.current;
      const committed = committedAttachmentBudget(itemsRef.current, artifactsRef.current);
      setComposerItems([...itemsRef.current, staging]);
      fileReadersRef.current.set(localId, () => planStageUpload(file, sessionId, committed));
      readAndUpload(localId, draft.sourceKind, sessionId, staging.operationToken, generation);
    }
  }, [makeLocalId, readAndUpload, setComposerItems]);

  const stageFromEvent = useCallback((event: AttachmentFileEvent) => {
    event.preventDefault?.();
    stageFiles(event.dataTransfer?.files ?? event.clipboardData?.files ?? null);
  }, [stageFiles]);

  const runConvert = useCallback((localId: string, token: number, sourceId: string, sessionId: string) => {
    const generation = generationRef.current;
    convertStagedNcm({ sessionId, attachmentId: sourceId, personaId: options.personaId }, host)
      .then((ready) => {
        const current = findItem(itemsRef.current, localId);
        const artifact = toGeneratedArtifact(ready, sourceId);
        if (!mountedRef.current || generationRef.current !== generation || current?.kind !== "processing" || current.operationToken !== token) {
          discardBackend(sessionId, artifact.id);
          return;
        }
        const next = reduceAttachmentState(current, { type: "conversionSucceeded", operationToken: token, artifact });
        if (next.kind !== "artifact-ready") return;
        setComposerItems(itemsRef.current.filter((item) => item.localId !== localId));
        setArtifactItems([...artifactsRef.current, next]);
        onArtifact({ state: next });
      })
      .catch((error: unknown) => {
        if (!mountedRef.current || generationRef.current !== generation) return;
        dispatchItem(localId, { type: "conversionFailed", operationToken: token, message: errorMessage(error) });
      });
  }, [discardBackend, dispatchItem, host, onArtifact, options.personaId, setArtifactItems, setComposerItems]);

  const confirm = useCallback((localId: string) => {
    const current = findItem(itemsRef.current, localId);
    if (current?.kind !== "awaiting-confirmation") return;
    const token = current.operationToken + 1;
    const sessionId = sessionIdRef.current;
    dispatchItem(localId, { type: "confirm" });
    runConvert(localId, token, current.source.id, sessionId);
  }, [dispatchItem, runConvert]);

  const cancel = useCallback((localId: string) => {
    if (preparationRef.current?.localIds.has(localId)) preparationRef.current.controller.abort();
    discardCurrentSource(findItem(itemsRef.current, localId), sessionIdRef.current, discardBackend);
    retryUploadsRef.current.delete(localId);
    fileReadersRef.current.delete(localId);
    releasePreview(localId);
    dispatchItem(localId, { type: "cancel" });
  }, [discardBackend, dispatchItem, releasePreview]);

  const remove = useCallback((localId: string) => {
    if (preparationRef.current?.localIds.has(localId)) preparationRef.current.controller.abort();
    discardCurrentSource(findItem(itemsRef.current, localId), sessionIdRef.current, discardBackend);
    retryUploadsRef.current.delete(localId);
    fileReadersRef.current.delete(localId);
    releasePreview(localId);
    dispatchItem(localId, { type: "remove" });
  }, [discardBackend, dispatchItem, releasePreview]);

  const runExport = useCallback((artifactId: string, token: number) => {
    const generation = generationRef.current;
    exportChatArtifact({ sessionId: sessionIdRef.current, artifactId }, host)
      .then((receipt) => {
        if (!mountedRef.current || generationRef.current !== generation) return;
        setArtifactItems(artifactsRef.current.map((item) => (
          item.artifact.id === artifactId
            ? reduceArtifact(item, { type: "exportSucceeded", operationToken: token, destinationName: receipt.fileName })
            : item
        )));
      })
      .catch((error: unknown) => {
        if (!mountedRef.current || generationRef.current !== generation) return;
        setArtifactItems(artifactsRef.current.map((item) => (
          item.artifact.id === artifactId
            ? reduceArtifact(item, { type: "exportFailed", operationToken: token, message: errorMessage(error) })
            : item
        )));
      });
  }, [host, setArtifactItems]);

  const retry = useCallback((localId: string) => {
    const current = findItem(itemsRef.current, localId);
    if (current?.kind !== "failed") return;
    const token = current.operationToken + 1;
    dispatchItem(localId, { type: "retry" });
    switch (current.phase) {
      case "staging": {
        const retryUpload = retryUploadsRef.current.get(localId);
        if (retryUpload) runUpload(localId, retryUpload, token, generationRef.current);
        else readAndUpload(localId, current.draft.sourceKind, sessionIdRef.current, token, generationRef.current);
        return;
      }
      case "conversion":
        runConvert(localId, token, current.source.id, sessionIdRef.current);
        return;
      case "export":
        runExport(current.artifact.id, token);
        return;
      default: {
        const exhaustive: never = current;
        return exhaustive;
      }
    }
  }, [dispatchItem, readAndUpload, runConvert, runExport, runUpload]);

  const download = useCallback((artifactId: string) => {
    const current = findArtifact(artifactsRef.current, artifactId);
    if (current?.kind !== "artifact-ready") return;
    const next = reduceArtifact(current, { type: "exportStarted" });
    setArtifactItems(artifactsRef.current.map((item) => (item === current ? next : item)));
    runExport(artifactId, next.operationToken);
  }, [runExport, setArtifactItems]);

  const retryDownload = useCallback((artifactId: string) => {
    const current = findArtifact(artifactsRef.current, artifactId);
    if (current?.kind !== "failed" || current.phase !== "export") return;
    const next = reduceArtifact(current, { type: "retry" });
    setArtifactItems(artifactsRef.current.map((item) => (item === current ? next : item)));
    runExport(artifactId, next.operationToken);
  }, [runExport, setArtifactItems]);

  const cleanupSession = useCallback(async (sessionId: string) => {
    if (sessionId.trim().length === 0) return;
    try {
      await cleanupChatSession({ sessionId }, host);
    } catch (error: unknown) {
      console.error(
        "chat attachment cleanup failed",
        error instanceof Error ? error : new Error(String(error)),
      );
      onBackgroundError(errorMessage(error));
    }
  }, [host, onBackgroundError]);

  const resetSession = useCallback((sessionId: string) => {
    if (sessionIdRef.current === sessionId) return;
    generationRef.current += 1;
    releaseAllPreviews();
    preparationRef.current?.controller.abort();
    sessionIdRef.current = sessionId;
    retryUploadsRef.current.clear();
    fileReadersRef.current.clear();
    setComposerItems(itemsRef.current.map((item) => reduceAttachmentState(item, { type: "sessionReset" })));
    setArtifactItems([]);
  }, [releaseAllPreviews, setArtifactItems, setComposerItems]);

  const discardSentSources = useCallback((localIds: readonly string[]) => {
    const sent = new Set(localIds);
    const currentSessionId = sessionIdRef.current;
    const remaining: AttachmentLifecycleState[] = [];
    for (const item of itemsRef.current) {
      if (item.kind === "ready" && sent.has(item.localId)) {
        discardBackend(currentSessionId, item.source.id);
        retryUploadsRef.current.delete(item.localId);
        releasePreview(item.localId);
        continue;
      }
      remaining.push(item);
    }
    setComposerItems(remaining);
  }, [discardBackend, releasePreview, setComposerItems]);

  const prepareModelAttachments = useCallback(async (message: string) => {
    preparationRef.current?.controller.abort();
    const generation = generationRef.current;
    const readyItems = itemsRef.current.filter((item) => item.kind === "ready");
    const preparation = {
      controller: new AbortController(),
      localIds: new Set(readyItems.map((item) => item.localId)),
    };
    preparationRef.current = preparation;
    const { signal } = preparation.controller;
    const sessionId = sessionIdRef.current;
    try {
      const fileParts = await Promise.all(readyItems.map(async (item) => {
        const ready = await readChatAttachment({ sessionId, attachmentId: item.source.id }, host);
        signal.throwIfAborted();
        return prepareOpenCodeFilePart(ready, signal);
      }));
      signal.throwIfAborted();
      return buildPreparedModelAttachments(message, fileParts);
    } catch (error: unknown) {
      // A failed attachment also stops other PDFs still being prepared for this send.
      preparation.controller.abort();
      throw error;
    } finally {
      if (preparationRef.current === preparation) preparationRef.current = null;
      if (mountedRef.current && generationRef.current === generation) {
        for (const localId of preparation.localIds) releasePreview(localId);
        setComposerItems(itemsRef.current.map((item) => {
          if (item.kind !== "ready" || !preparation.localIds.has(item.localId) || !item.source.previewDataUrl) return item;
          const { previewDataUrl: _preview, ...source } = item.source;
          return { ...item, source };
        }));
      }
    }
  }, [host, releasePreview, setComposerItems]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
      preparationRef.current?.controller.abort();
      retryUploadsRef.current.clear();
      fileReadersRef.current.clear();
      releaseAllPreviews();
    };
  }, [releaseAllPreviews]);

  useEffect(() => {
    if (options.sessionId !== sessionIdRef.current) resetSession(options.sessionId);
  }, [options.sessionId, resetSession]);

  return {
    items,
    artifacts,
    stageFromPicker: stageFiles,
    stageFromDrop: stageFromEvent,
    stageFromPaste: stageFromEvent,
    confirm,
    cancel,
    remove,
    retry,
    download,
    retryDownload,
    resetSession,
    cleanupSession,
    discardSentSources,
    prepareModelAttachments,
  };
}
