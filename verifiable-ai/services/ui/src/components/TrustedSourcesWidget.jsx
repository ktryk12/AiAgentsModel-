export function TrustedSourcesWidget({ status, onManage }) {
    const count = status?.count ?? 0;
    const lastUpdated = status?.last_updated ?? null;

    return (
        <div className="tsWidget glass">
            <div className="tsTop">
                <div className="tsTitle">🔒 Your Trusted Sources</div>
                <button className="btn ghost" onClick={onManage}>Manage</button>
            </div>

            <div className="tsBody">
                <div className="tsLine">
                    <span className="tsKey">Connected</span>
                    <span className="tsVal">{count} sources</span>
                </div>
                <div className="tsLine">
                    <span className="tsKey">Status</span>
                    <span className={`tsVal ${count > 0 ? "ok" : "warn"}`}>
                        {count > 0 ? "Active (Always On)" : "Not connected yet"}
                    </span>
                </div>
                <div className="tsHint">
                    {count > 0
                        ? `Last updated: ${lastUpdated ?? "recently"}`
                        : "Add trusted sources to keep answers grounded and consistent."}
                </div>
            </div>
        </div>
    );
}
