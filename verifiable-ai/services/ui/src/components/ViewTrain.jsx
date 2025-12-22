import { useState } from "react";
import { api } from "../api";

export function ViewTrain({ onBack }) {
    const [step, setStep] = useState(1);
    const [type, setType] = useState("llm"); // llm | image
    const [dataset, setDataset] = useState("");
    const [baseModel, setBaseModel] = useState("llama3-8b");
    const [submitting, setSubmitting] = useState(false);
    const [jobId, setJobId] = useState(null);

    async function submit() {
        setSubmitting(true);
        try {
            const kind = type === "llm" ? "train.llm" : "train.image_lora";
            // Mock submission for Phase 15
            const res = await api.postJob({
                kind,
                queue: "training_queue",
                payload: { type, dataset, baseModel }
            });
            const id = res?.id ?? res?.job_id;
            if (id) setJobId(id);
            setStep(4); // Done
        } catch (e) {
            alert(e.message);
        } finally {
            setSubmitting(false);
        }
    }

    return (
        <div className="wizard glass">
            <div className="wizardHeader">
                <button className="btn ghost" onClick={onBack}>← Back</button>
                <div className="wizardTitle">Train a Model</div>
            </div>

            <div className="wizardBody">
                {step === 1 && (
                    <div className="stepAnim">
                        <h2>Step 1: Choose Model Type</h2>
                        <div className="cardGrid">
                            <div className={`selCard ${type === "llm" ? "active" : ""}`} onClick={() => setType("llm")}>
                                <div className="icon">📝</div>
                                <h3>Text Model (LLM)</h3>
                                <p>Fine-tune for code, chat, or specific writing styles.</p>
                            </div>
                            <div className={`selCard ${type === "image" ? "active" : ""}`} onClick={() => setType("image")}>
                                <div className="icon">🎨</div>
                                <h3>Image Model (LoRA)</h3>
                                <p>Train on a style, character, or product.</p>
                            </div>
                        </div>
                        <div className="actions right">
                            <button className="btn" onClick={() => setStep(2)}>Next</button>
                        </div>
                    </div>
                )}

                {step === 2 && (
                    <div className="stepAnim">
                        <h2>Step 2: Data & Config</h2>
                        <label className="label">
                            <span className="labelText">Dataset (Zip/Folder)</span>
                            <input className="input" placeholder="/path/to/data.zip" value={dataset} onChange={e => setDataset(e.target.value)} />
                        </label>
                        {type === "llm" && (
                            <label className="label">
                                <span className="labelText">Base Model</span>
                                <select className="select" value={baseModel} onChange={e => setBaseModel(e.target.value)}>
                                    <option value="llama3-8b">Llama 3 (8B)</option>
                                    <option value="mistral-7b">Mistral (7B)</option>
                                </select>
                            </label>
                        )}
                        <div className="actions right">
                            <button className="btn ghost" onClick={() => setStep(1)}>Back</button>
                            <button className="btn" onClick={() => setStep(3)}>Next</button>
                        </div>
                    </div>
                )}

                {step === 3 && (
                    <div className="stepAnim">
                        <h2>Step 3: Confirm</h2>
                        <div className="confirmBox">
                            <div>Type: <b>{type === "llm" ? "Text (LLM)" : "Image (LoRA)"}</b></div>
                            <div>Dataset: <b>{dataset || "N/A"}</b></div>
                            {type === "llm" && <div>Model: <b>{baseModel}</b></div>}
                        </div>
                        <div className="actions right">
                            <button className="btn ghost" onClick={() => setStep(2)}>Back</button>
                            <button className="btn primary" onClick={submit} disabled={submitting}>
                                {submitting ? "Starting..." : "Start Training"}
                            </button>
                        </div>
                    </div>
                )}

                {step === 4 && (
                    <div className="stepAnim center">
                        <div className="icon big">🚀</div>
                        <h2>Training Started!</h2>
                        <p className="muted">Job ID: {jobId}</p>
                        <div className="actions">
                            <button className="btn" onClick={onBack}>Back to Studio</button>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
