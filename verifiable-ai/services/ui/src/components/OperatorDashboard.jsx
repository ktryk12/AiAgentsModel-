import { useMemo, useState } from "react";
import { api } from "../api";
import { usePoll } from "../hooks";
import { SchedulerHeader } from "./SchedulerHeader";
import { QueueCards } from "./QueueCards";
import { JobTable } from "./JobTable";
import { JobDrawer } from "./JobDrawer";

export function OperatorDashboard() {
    const scheduler = usePoll(api.getScheduler, 2000, []);
    const jobs = usePoll(api.getJobs, 2000, []);

    const [selectedId, setSelectedId] = useState(null);

    const queuesObj = useMemo(() => scheduler.data?.queues ?? {}, [scheduler.data]);
    const queueNames = useMemo(() => Object.keys(queuesObj), [queuesObj]);

    return (
        <>
            <header className="header glass">
                <div className="titleRow">
                    <div>
                        <div className="title">Operator Dashboard</div>
                        <div className="subtitle">Live scheduler + jobs + lifecycle control</div>
                    </div>
                    <div className="pillRow">
                        <span className="pill">API: {import.meta.env.VITE_API_URL}</span>
                    </div>
                </div>

                <SchedulerHeader state={scheduler} />
                <QueueCards queues={queuesObj} />
            </header>

            <main className="main">
                <section className="card glass">
                    <div className="cardHeader">
                        <div className="cardTitle">Jobs</div>
                        <div className="cardHint">Auto-refresh (paused when tab hidden)</div>
                    </div>

                    <JobTable
                        state={jobs}
                        queues={queueNames}
                        onSelect={setSelectedId}
                        onCancel={async (id) => { await api.cancelJob(id); await jobs.refresh(); }}
                        onRetry={async (id) => { await api.retryJob(id); await jobs.refresh(); }}
                    />
                </section>
            </main>

            <JobDrawer
                jobId={selectedId}
                onClose={() => setSelectedId(null)}
                onCancel={async (id) => { await api.cancelJob(id); }}
                onRetry={async (id) => { await api.retryJob(id); }}
            />
        </>
    );
}
