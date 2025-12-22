function pct(running, cap) {
    if (!cap || cap <= 0) return 0;
    return Math.min(100, Math.round((running / cap) * 100));
}

export function QueueCards({ queues }) {
    const entries = Object.entries(queues);

    if (!entries.length) return null;

    return (
        <div className="queueGrid">
            {entries.map(([name, q]) => {
                const running = q.running ?? 0;
                const pending = q.pending ?? 0;
                const cap = q.cap ?? 0;
                const p = pct(running, cap);
                return (
                    <div key={name} className="queueCard glass">
                        <div className="queueTop">
                            <div className="queueName">{name}</div>
                            <div className="queueCap">cap {cap}</div>
                        </div>
                        <div className="queueBar">
                            <div className="queueFill" style={{ width: `${p}%` }} />
                        </div>
                        <div className="queueBottom">
                            <span className="kv">running <b>{running}</b></span>
                            <span className="kv">pending <b>{pending}</b></span>
                            <span className="kv">{p}%</span>
                        </div>
                    </div>
                );
            })}
        </div>
    );
}
