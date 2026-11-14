import { useEffect, useRef, useState } from "react";
import { getReportSources, onWorklogChanged, type ReportSourcePage, type ReportVersionContent } from "../../lib/worklog";
import type { WorklogLabels } from "./worklogLabels";

function SourcePage({ version, labels }: { readonly version: ReportVersionContent; readonly labels: WorklogLabels }) {
  const [offset, setOffset] = useState(0);
  const [refresh, setRefresh] = useState(0);
  const [result, setResult] = useState<{ offset: number; page: ReportSourcePage } | null>(null);
  const [failed, setFailed] = useState(false);
  const generation = useRef(0);
  useEffect(() => {
    const request = ++generation.current;
    void getReportSources(version.reportId, version.id, offset).then(page => {
      if (request === generation.current) { setResult({ offset, page }); setFailed(false); }
    }, () => { if (request === generation.current) setFailed(true); });
    return () => { generation.current += 1; };
  }, [version.reportId, version.id, offset, refresh]);
  useEffect(() => {
    let closed = false;
    const subscription = onWorklogChanged(() => {
      if (!closed) { generation.current += 1; setResult(null); setOffset(0); setRefresh(value => value + 1); }
    });
    void subscription.catch(() => { if (!closed) setFailed(true); });
    return () => { closed = true; void subscription.then(unlisten => unlisten(), () => undefined); };
  }, []);
  const page = result?.offset === offset ? result.page : null;
  return <div>
    {failed && <button className="set-btn" type="button" onClick={() => { setFailed(false); setRefresh(value => value + 1); }}>{labels.retry}</button>}
    {!page && !failed && <p role="status">{labels.loading}</p>}
    {page && <><ul className="worklog-list">{page.sources.map(source => <li className="worklog-item" key={`${source.source.kind}:${source.source.id}`}>
      <p className="worklog-meta">{[source.businessDate, source.project, source.source.id, `r${source.source.revision}`].filter(Boolean).join(" · ")}</p><p className="worklog-source-text">{source.text}</p>
    </li>)}</ul><div className="worklog-actions">
      <button className="set-btn" type="button" disabled={offset === 0} onClick={() => { setFailed(false); setOffset(Math.max(0, offset - 50)); }}>← {labels.sources}</button>
      <span>{offset + (page.sources.length ? 1 : 0)}–{offset + page.sources.length}</span>
      <button className="set-btn" type="button" disabled={page.nextOffset === null} onClick={() => { if (page.nextOffset !== null) { setFailed(false); setOffset(page.nextOffset); } }}>{labels.sources} →</button>
    </div></>}
  </div>;
}

export function ReportSources({ version, missing, labels }: { readonly version: ReportVersionContent; readonly missing: readonly string[]; readonly labels: WorklogLabels }) {
  const [open, setOpen] = useState(false);
  return <details onToggle={event => setOpen(event.currentTarget.open)}><summary>{labels.sources} · {version.sourceCount}</summary>
    {open && <>{missing.length > 0 && <p className="worklog-warning">{labels.missing}: {missing.join(", ")}</p>}<SourcePage version={version} labels={labels} /></>}
  </details>;
}
