export function SchedulerHeader({ state }) {
    const { data, error, loading } = state;

    if (loading) return <div className="row muted">Loading scheduler…</div>;
    if (error) return <div className="row err">Scheduler error: {String(error.message || error)}</div>;

    return (
        <div className="row metrics">
            <div className="metric">
                <div className="metricLabel">Running</div>
                <div className="metricValue">{data.running ?? 0}</div>
            </div>
            <div className="metric">
                <div className="metricLabel">Pending</div>
                <div className="metricValue">{data.pending ?? 0}</div>
            </div>
            <div className="metric">
                <div className="metricLabel">Locked datasets</div>
                <div className="metricValue">{data.locked_datasets ?? 0}</div>
            </div>
            <div className="metric">
                <div className="metricLabel">Workers active</div>
                <div className="metricValue">{data.workers_active ?? 0}</div>
            </div>
            <div className="metric">
                <div className="metricLabel">Capacity</div>
                <div className="metricValue">{data.capacity_pct ?? 0}%</div>
            </div>
        </div>
    );
}
