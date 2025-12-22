import { useState } from "react";
import { api } from "../api";

export function ViewKnowledge({ onBack }) {
    const [submitting, setSubmitting] = useState(false);
    const [path, setPath] = useState("");
    const [done, setDone] = useState(false);

    async function submit() {
        setSubmitting(true);
        try {
            await api.postJob({
                kind: "kb.create",
                queue: "default",
                payload: { path, index_name: "main_index" }
            });
            setDone(true);
        } catch (e) {
            alert(e.message);
        } finally {
            setSubmitting(false);
        }
    }

    if (done) {
        return (
            <div className="wizard glass center">
                <div className="icon big">📚</div>
                <h2>Knowledge Base Indexing</h2>
                <p>Your documents are being processed.</p>
                <button className="btn" onClick={onBack}>Done</button>
            </div>
        );
    }

    return (
        <div className="wizard glass">
            <div className="wizardHeader">
                <button className="btn ghost" onClick={onBack}>← Back</button>
                <div className="wizardTitle">Connect Knowledge</div>
            </div>
            <div className="wizardBody">
                <p className="desc">
                    Index PDFs, Markdown, or Text files to let your agents reference your specific knowledge.
                </p>
                <label className="label">
                    <span className="labelText">Source Path (Folder or File)</span>
                    <input className="input" placeholder="e.g. C:\Users\Admin\Documents\ProjectX" value={path} onChange={e => setPath(e.target.value)} />
                </label>
                <div className="actions right">
                    <button className="btn primary" onClick={submit} disabled={submitting || !path}>
                        {submitting ? "Indexing..." : "Start Indexing"}
                    </button>
                </div>
            </div>
        </div>
    );
}
