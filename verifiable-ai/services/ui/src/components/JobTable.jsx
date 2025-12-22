import { useMemo, useState } from "react";

function short(id) {
    return String(id).slice(0, 8);
}

function badgeClass(status) {
    switch (status) {
        case "running": return "badge running";
        case "pending": return "badge pending";
        case "done": return "badge done";
        case "failed": return "badge failed";
        case "cancelled": return "badge cancelled";
        default: return "badge";
    }
}

function timeAgo(iso) {
    if (!iso) return "";
    const ms = Date.now() - new Date(iso).getTime();
    const s = Math.max(0, Math.floor(ms / 1000));
    if (s < 60) return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    return `${h}h ago`;
}

function norm(v) {
    return String(v ?? "").toLowerCase().trim();
}

function jobDatasetId(j) {
    // try common shapes; harmless if undefined
    return j.dataset_id ?? j.payload?.dataset_id ?? j.payload?.datasetId;
}

export function JobTable({ state, onSelect, onCancel, onRetry, queues = [] }) {
    const { data, error, loading } = state;

    const [q, setQ] = useState("");
    const [status, setStatus] = useState("all");
    const [queue, setQueue] = useState("all");

    const rows = Array.isArray(data) ? data : (data?.jobs ?? []);

    const filtered = useMemo(() => {
        const qq = norm(q);

        return rows.filter((j) => {
            if (status !== "all" && norm(j.status) !== status) return false;
            if (queue !== "all" && norm(j.queue) !== queue) return false;

            if (!qq) return true;

            const hay = [
                j.id,
                j.kind,
                j.queue,
                j.status,
                jobDatasetId(j),
            ].map(norm).join(" ");

            return hay.includes(qq);
        });
    }, [rows, q, status, queue]);

    if (loading) return <div className="muted pad">Loading jobs…</div>;
    if (error) return <div className="err pad">Jobs error: {String(error.message || error)}</div>;

    return (
        <>
            <div className="filterBar">
                <input
                    className="input"
                    placeholder="Search: id / kind / dataset / queue…"
                    value={q}
                    onChange={(e) => setQ(e.target.value)}
                />

                <select className="select" value={status} onChange={(e) => setStatus(e.target.value)}>
                    <option value="all">Status: All</option>
                    <option value="pending">Pending</option>
                    <option value="running">Running</option>
                    <option value="done">Done</option>
                    <option value="failed">Failed</option>
                    <option value="cancelled">Cancelled</option>
                </select>

                <select className="select" value={queue} onChange={(e) => setQueue(e.target.value)}>
                    <option value="all">Queue: All</option>
                    {queues.map((name) => (
                        <option key={name} value={norm(name)}>{name}</option>
                    ))}
                </select>

                <div className="filterMeta muted">
                    Showing <b>{filtered.length}</b> / {rows.length}
                </div>
            </div>

            <div className="tableWrap">
                <table className="table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Kind</th>
                            <th>Queue</th>
                            <th>Status</th>
                            <th>Priority</th>
                            <th>Attempts</th>
                            <th>Created</th>
                            <th className="right">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {filtered.map((j) => (
                            <tr key={j.id} onClick={() => onSelect(j.id)} className="rowClick">
                                <td className="mono">{short(j.id)}</td>
                                <td>{j.kind}</td>
                                <td className="mono">{j.queue ?? "-"}</td>
                                <td><span className={badgeClass(j.status)}>{j.status}</span></td>
                                <td className="mono">{j.priority ?? 0}</td>
                                <td className="mono">{j.attempts ?? 0}</td>
                                <td className="muted">{timeAgo(j.created_at)}</td>
                                <td className="right" onClick={(e) => e.stopPropagation()}>
                                    <button
                                        className="btn"
                                        onClick={() => onCancel(j.id)}
                                        disabled={j.status !== "running" && j.status !== "pending"}
                                    >
                                        Cancel
                                    </button>
                                    <button
                                        className="btn ghost"
                                        onClick={() => onRetry(j.id)}
                                        disabled={j.status !== "failed" && j.status !== "cancelled"}
                                    >
                                        Retry
                                    </button>
                                </td>
                            </tr>
                        ))}

                        {!filtered.length && (
                            <tr><td colSpan="8" className="muted pad">No matching jobs.</td></tr>
                        )}
                    </tbody>
                </table>
            </div>
        </>
    );
}
