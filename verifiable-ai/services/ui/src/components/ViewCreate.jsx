import { useState } from "react";
import { api } from "../api";
import { StudioCard } from "./StudioCard";
import { VerifiedBadge } from "./VerifiedBadge";

function isTerminal(status) {
    return status === "done" || status === "failed" || status === "cancelled";
}

function extractResult(job) {
    if (job?.result != null) return job.result;
    if (job?.output != null) return job.output;
    if (job?.payload?.result != null) return job.payload.result;

    const evs = Array.isArray(job?.events) ? job.events : [];
    for (let i = evs.length - 1; i >= 0; i--) {
        const ev = evs[i];
        if (ev?.result != null) return ev.result;
        if (ev?.output != null) return ev.output;
        if (ev?.text != null) return ev.text;
        if (ev?.image_url != null) return { image_url: ev.image_url };
        if (ev?.url != null) return { url: ev.url };
    }
    return null;
}

export function ViewCreate({ onBack }) {
    // submode: 'menu' | 'text' | 'images' | 'assistants' | 'run' | 'result'
    const [mode, setMode] = useState("menu");
    const [error, setError] = useState("");

    // Forms
    const [textPrompt, setTextPrompt] = useState("");
    const [textQuality, setTextQuality] = useState("normal");
    const [imgPrompt, setImgPrompt] = useState("");
    const [imgStyle, setImgStyle] = useState("normal");

    // Assistant Forms
    const [agentSkill, setAgentSkill] = useState("pc_helper");
    const [agentGoal, setAgentGoal] = useState("");

    // Job state
    const [jobId, setJobId] = useState(null);
    const [jobStatus, setJobStatus] = useState(null);
    const [result, setResult] = useState(null);

    async function submitJob(job) {
        setError("");
        setMode("run");
        setResult(null);
        try {
            const created = await api.postJob(job);
            const id = created?.id ?? created?.job_id;
            if (!id) throw new Error("No job ID returned");
            setJobId(id);

            while (true) {
                const j = await api.getJob(id);
                const st = j?.status ?? "pending";
                setJobStatus(st);
                const r = extractResult(j);
                if (r != null) setResult(r);

                if (isTerminal(st)) {
                    if (st === "failed" || st === "cancelled") setError(`Job ${st}`);
                    setMode("result");
                    return;
                }
                await new Promise(r => setTimeout(r, 1000));
            }
        } catch (e) {
            setError(e.message);
            setMode("result");
        }
    }

    function renderMenu() {
        return (
            <div className="studioGrid">
                <StudioCard icon="✍️" title="Text" desc="Write content & emails" onClick={() => setMode("text")} />
                <StudioCard icon="🖼️" title="Images" desc="Generate visuals" onClick={() => setMode("images")} />
                <StudioCard icon="🤖" title="Assistants" desc="PC Helper, Email, Research" onClick={() => setMode("assistants")} />
            </div>
        );
    }

    function renderTextForm() {
        return (
            <div className="studioForm">
                <label className="label">
                    <span className="labelText">Prompt</span>
                    <textarea className="textarea" rows={5} value={textPrompt} onChange={e => setTextPrompt(e.target.value)} />
                </label>
                <div className="studioRow">
                    <div className="radioGroup">
                        <label className="radio"><input type="radio" checked={textQuality === "normal"} onChange={() => setTextQuality("normal")} /> Normal</label>
                        <label className="radio"><input type="radio" checked={textQuality === "best"} onChange={() => setTextQuality("best")} /> Best</label>
                    </div>
                    <button className="btn primary" onClick={() => submitJob({
                        kind: "text_generation",
                        queue: "default",
                        payload: { prompt: textPrompt, quality: textQuality }
                    })}>Generate</button>
                </div>
            </div>
        );
    }

    function renderImageForm() {
        return (
            <div className="studioForm">
                <label className="label">
                    <span className="labelText">Image Prompt</span>
                    <textarea className="textarea" rows={5} value={imgPrompt} onChange={e => setImgPrompt(e.target.value)} />
                </label>
                <div className="studioRow">
                    <div className="radioGroup">
                        <label className="radio"><input type="radio" checked={imgStyle === "normal"} onChange={() => setImgStyle("normal")} /> Normal</label>
                        <label className="radio"><input type="radio" checked={imgStyle === "vivid"} onChange={() => setImgStyle("vivid")} /> Vivid</label>
                    </div>
                    <button className="btn primary" onClick={() => submitJob({
                        kind: "image_generation",
                        queue: "gpu_queue",
                        payload: { prompt: imgPrompt, style: imgStyle }
                    })}>Generate</button>
                </div>
            </div>
        );
    }

    function renderAssistantForm() {
        return (
            <div className="studioForm">
                <label className="label">
                    <span className="labelText">Skill / Agent Type</span>
                    <select className="select" value={agentSkill} onChange={e => setAgentSkill(e.target.value)}>
                        <option value="pc_helper">PC Helper (Installs, Tweaks)</option>
                        <option value="email_connector">Email Assistant</option>
                        <option value="research">Research / Writing</option>
                    </select>
                </label>
                <label className="label">
                    <span className="labelText">Goal / Instruction</span>
                    <textarea className="textarea" rows={5} placeholder="e.g. Install WSL2 or Summarize my inbox" value={agentGoal} onChange={e => setAgentGoal(e.target.value)} />
                </label>
                <div className="studioRow">
                    <div className="muted" style={{ fontSize: 12 }}>Safe Mode: Actions will require approval.</div>
                    <button className="btn primary" onClick={() => submitJob({
                        kind: "agent.run",
                        queue: "agent_queue",
                        payload: { skill: agentSkill, goal: agentGoal }
                    })}>Start Agent</button>
                </div>
            </div>
        );
    }

    if (mode === "run") {
        return <div className="wizard glass center"><div className="spinner" /><h3>Processing...</h3></div>;
    }

    if (mode === "result") {
        return (
            <div className="wizard glass">
                <div className="wizardHeader">
                    <div className="wizardTitle">Result</div>
                    <button className="btn ghost" onClick={() => setMode("menu")}>Close</button>
                </div>
                {error && <div className="err pad">{error}</div>}
                <div style={{ padding: '0 20px' }}><VerifiedBadge /></div>
                <pre className="code">{JSON.stringify(result, null, 2)}</pre>
            </div>
        );
    }

    return (
        <div className="wizard glass">
            <div className="wizardHeader">
                <button className="btn ghost" onClick={mode === "menu" ? onBack : () => setMode("menu")}>← Back</button>
                <div className="wizardTitle">Create Content</div>
            </div>
            <div className="wizardBody">
                {mode === "menu" && renderMenu()}
                {mode === "text" && renderTextForm()}
                {mode === "images" && renderImageForm()}
                {mode === "assistants" && renderAssistantForm()}
            </div>
        </div>
    );
}
