import { useEffect, useMemo, useState } from "react";
import { api } from "../api";

function safeParseJson(text) {
    try {
        const v = JSON.parse(text);
        return { ok: true, value: v };
    } catch (e) {
        return { ok: false, error: e };
    }
}

export function CreateJobModal({ open, onClose, queues = [], onCreated }) {
    const [kind, setKind] = useState("train");
    const [queue, setQueue] = useState(queues[0] ?? "");
    const [priority, setPriority] = useState(0);
    const [payloadText, setPayloadText] = useState("{\n  \n}");
    const [submitting, setSubmitting] = useState(false);
    const [err, setErr] = useState("");

    useEffect(() => {
        if (open) {
            setErr("");
            setQueue((q) => q || (queues[0] ?? ""));
        }
    }, [open, queues]);

    const parsed = useMemo(() => safeParseJson(payloadText), [payloadText]);

    if (!open) return null;

    const canSubmit =
        kind.trim().length > 0 &&
        (queue?.trim()?.length ?? 0) > 0 &&
        parsed.ok &&
        !submitting;

    async function submit() {
        setErr("");
        if (!parsed.ok) {
            setErr("Payload must be valid JSON.");
            return;
        }

        setSubmitting(true);
        try {
            const job = {
                kind: kind.trim(),
                queue: queue.trim(),
                priority: Number(priority) || 0,
                payload: parsed.value,
            };

            const created = await api.postJob(job);
            onCreated?.(created);
            onClose();
        } catch (e) {
            setErr(String(e.message || e));
        } finally {
            setSubmitting(false);
        }
    }

    return (
        <div className="modalOverlay" onClick={onClose}>
            <div className="modal glass" onClick={(e) => e.stopPropagation()}>
                <div className="modalHeader">
                    <div>
                        <div className="modalTitle">Create Job</div>
                        <div className="modalSub muted">POST /training/jobs</div>
                    </div>
                    <button className="btn ghost" onClick={onClose}>Close</button>
                </div>

                {err && <div className="err pad">{err}</div>}

                <div className="formGrid">
                    <label className="label">
                        <span className="labelText">Kind</span>
                        <input className="input" value={kind} onChange={(e) => setKind(e.target.value)} />
                    </label>

                    <label className="label">
                        <span className="labelText">Queue</span>
                        <select className="select" value={queue} onChange={(e) => setQueue(e.target.value)}>
                            {queues.map((q) => <option key={q} value={q}>{q}</option>)}
                        </select>
                    </label>

                    <label className="label">
                        <span className="labelText">Priority</span>
                        <input
                            className="input"
                            type="number"
                            value={priority}
                            onChange={(e) => setPriority(e.target.value)}
                        />
                    </label>

                    <label className="label labelFull">
                        <span className="labelText">
                            Payload (JSON)
                            {!parsed.ok && <span className="errInline"> invalid JSON</span>}
                        </span>
                        <textarea
                            className="textarea mono"
                            rows={12}
                            value={payloadText}
                            onChange={(e) => setPayloadText(e.target.value)}
                        />
                    </label>
                </div>

                <div className="modalActions">
                    <button className="btn ghost" onClick={onClose}>Cancel</button>
                    <button className="btn" onClick={submit} disabled={!canSubmit}>
                        {submitting ? "Creating…" : "Create Job"}
                    </button>
                </div>
            </div>
        </div>
    );
}
