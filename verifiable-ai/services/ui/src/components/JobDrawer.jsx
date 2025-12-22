import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { usePoll } from "../hooks";

function fmtTs(iso) {
    if (!iso) return "";
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return String(iso);
    return d.toLocaleString();
}

function eventTs(ev) {
    return ev.timestamp ?? ev.ts ?? ev.time ?? ev.created_at ?? ev.at;
}

function eventType(ev) {
    return ev.type ?? ev.event_type ?? ev.name ?? "event";
}

function normalizeEvents(events) {
    if (!Array.isArray(events)) return [];
    // sort ascending by timestamp if possible
    return [...events].sort((a, b) => {
        const ta = new Date(eventTs(a) ?? 0).getTime();
        const tb = new Date(eventTs(b) ?? 0).getTime();
        return (ta || 0) - (tb || 0);
    });
}

export function JobDrawer({ jobId, onClose, onCancel, onRetry }) {
    const enabled = Boolean(jobId);

    const state = usePoll(
        () => (enabled ? api.getJob(jobId) : Promise.resolve(null)),
        1000,
        [jobId]
    );

    const { data, error, loading } = state;

    const events = useMemo(() => normalizeEvents(data?.events), [data]);

    const [expanded, setExpanded] = useState(() => new Set());
    const [autoScroll, setAutoScroll] = useState(true);
    const eventsRef = useRef(null);

    useEffect(() => {
        // reset expansion on job change
        setExpanded(new Set());
        setAutoScroll(true);
    }, [jobId]);

    useEffect(() => {
        if (!autoScroll) return;
        const el = eventsRef.current;
        if (!el) return;
        el.scrollTop = el.scrollHeight;
    }, [events.length, autoScroll]);

    function toggle(i) {
        setExpanded((prev) => {
            const next = new Set(prev);
            if (next.has(i)) next.delete(i);
            else next.add(i);
            return next;
        });
    }

    if (!enabled) return null;

    return (
        <div className="drawerOverlay" onClick={onClose}>
            <div className="drawer glass" onClick={(e) => e.stopPropagation()}>
                <div className="drawerHeader">
                    <div>
                        <div className="drawerTitle">Job {String(jobId).slice(0, 8)}</div>
                        <div className="drawerSub">Auto-refresh every 1s (paused when tab hidden)</div>
                    </div>
                    <button className="btn ghost" onClick={onClose}>Close</button>
                </div>

                {loading && <div className="muted pad">Loading…</div>}
                {error && <div className="err pad">Error: {String(error.message || error)}</div>}

                {data && (
                    <>
                        <div className="drawerActions">
                            <button
                                className="btn"
                                onClick={() => onCancel(jobId)}
                                disabled={data.status !== "running" && data.status !== "pending"}
                            >
                                Cancel
                            </button>
                            <button
                                className="btn ghost"
                                onClick={() => onRetry(jobId)}
                                disabled={data.status !== "failed" && data.status !== "cancelled"}
                            >
                                Retry
                            </button>
                        </div>

                        <div className="kvGrid">
                            <div className="kvItem"><span className="k">status</span><span className="v">{data.status}</span></div>
                            <div className="kvItem"><span className="k">queue</span><span className="v">{data.queue ?? "-"}</span></div>
                            <div className="kvItem"><span className="k">priority</span><span className="v">{data.priority ?? 0}</span></div>
                            <div className="kvItem"><span className="k">attempts</span><span className="v">{data.attempts ?? 0}</span></div>
                            <div className="kvItem"><span className="k">paused</span><span className="v">{String(data.paused ?? false)}</span></div>
                            <div className="kvItem"><span className="k">cancel_requested</span><span className="v">{String(data.cancel_requested ?? false)}</span></div>
                        </div>

                        <div className="split">
                            <div className="panel">
                                <div className="panelTitle">Payload</div>
                                <pre className="code">{JSON.stringify(data.payload ?? {}, null, 2)}</pre>
                            </div>

                            <div className="panel">
                                <div className="panelTitleRow">
                                    <div className="panelTitle">Events (Timeline)</div>
                                    <label className="toggle">
                                        <input
                                            type="checkbox"
                                            checked={autoScroll}
                                            onChange={(e) => setAutoScroll(e.target.checked)}
                                        />
                                        <span>Auto-scroll</span>
                                    </label>
                                </div>

                                <div className="timeline" ref={eventsRef}>
                                    {events.map((ev, i) => {
                                        const ts = eventTs(ev);
                                        const open = expanded.has(i);
                                        return (
                                            <div key={i} className="tlItem">
                                                <div className="tlRail" />
                                                <div className="tlDot" />

                                                <div className="tlBody">
                                                    <button className="tlHead" onClick={() => toggle(i)}>
                                                        <span className="tlType mono">{eventType(ev)}</span>
                                                        <span className="tlTs muted">{fmtTs(ts)}</span>
                                                        <span className="tlHint muted">{open ? "hide" : "show"} JSON</span>
                                                    </button>

                                                    {open && (
                                                        <pre className="tlJson">{JSON.stringify(ev, null, 2)}</pre>
                                                    )}
                                                </div>
                                            </div>
                                        );
                                    })}

                                    {!events.length && <div className="muted">No events.</div>}
                                </div>
                            </div>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
