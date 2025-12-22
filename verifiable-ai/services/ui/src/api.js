const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

async function http(path, opts = {}) {
    const res = await fetch(`${API_URL}${path}`, {
        headers: { "Content-Type": "application/json", ...(opts.headers ?? {}) },
        ...opts,
    });
    if (!res.ok) {
        const text = await res.text().catch(() => "");
        throw new Error(`${res.status} ${res.statusText}${text ? `: ${text}` : ""}`);
    }
    const ct = res.headers.get("content-type") || "";
    return ct.includes("application/json") ? res.json() : res.text();
}

export const api = {
    getScheduler: () => http("/training/scheduler"),
    getJobs: () => http("/training/jobs"),
    getJob: (id) => http(`/training/jobs/${id}`),

    postJob: (body) => http("/training/jobs", { method: "POST", body: JSON.stringify(body) }),

    cancelJob: (id) => http(`/training/jobs/${id}/cancel`, { method: "POST" }),
    retryJob: (id) => http(`/training/jobs/${id}/retry`, { method: "POST" }),

    // optional if enabled
    pauseJob: (id) => http(`/training/jobs/${id}/pause`, { method: "POST" }),
    resumeJob: (id) => http(`/training/jobs/${id}/resume`, { method: "POST" }),
};
