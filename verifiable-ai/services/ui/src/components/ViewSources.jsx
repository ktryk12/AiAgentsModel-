import { useState } from "react";
import { api } from "../api";

const DEFAULT_SCOPE = { answer: true, write: true, generate: true, train: false };

export function ViewSources({ onDone, onBack }) {
    const [step, setStep] = useState(1);
    const [error, setError] = useState("");

    const [sourceType, setSourceType] = useState("files"); // files|folder|website|projects
    const [sourceValue, setSourceValue] = useState("");   // path/url/name
    const [indexName, setIndexName] = useState("trusted-sources");

    const [scope, setScope] = useState(DEFAULT_SCOPE);
    const [running, setRunning] = useState(false);

    function toggleScope(k) {
        setScope((s) => ({ ...s, [k]: !s[k] }));
    }

    async function submit() {
        setError("");
        setRunning(true);
        try {
            // Minimal “kb.create” job
            const job = {
                kind: "kb.create",
                queue: "default",
                payload: {
                    source_type: sourceType,
                    source_path: sourceValue,  // (kan være url eller folder eller "project:xyz")
                    index_name: indexName,
                    scope,
                },
            };

            const created = await api.postJob(job);
            // I kan evt. poll’e job her, men “radical simplicity” kan nøjes med “we’re indexing”
            setStep(3);
            setRunning(false);
            return created;
        } catch (e) {
            setRunning(false);
            setError(String(e.message || e));
        }
    }

    return (
        <section className="card glass">
            <div className="cardHeader">
                <div className="cardTitle">Trusted Sources</div>
                <div className="cardHint">Always On grounding for answers & creation</div>
            </div>

            {error && <div className="err pad">{error}</div>}

            {step === 1 && (
                <div className="studioForm">
                    <div className="labelText">Step 1 — Choose a source</div>

                    <div className="pillSelect">
                        {["files", "folder", "website", "projects"].map((t) => (
                            <button
                                key={t}
                                className={`pillBtn ${sourceType === t ? "active" : ""}`}
                                onClick={() => setSourceType(t)}
                                type="button"
                            >
                                {t === "files" && "Files"}
                                {t === "folder" && "Folder"}
                                {t === "website" && "Website"}
                                {t === "projects" && "Projects"}
                            </button>
                        ))}
                    </div>

                    <label className="label">
                        <span className="labelText">
                            {sourceType === "website" ? "Website URL" : "Path / Reference"}
                        </span>
                        <input
                            className="input"
                            value={sourceValue}
                            onChange={(e) => setSourceValue(e.target.value)}
                            placeholder={
                                sourceType === "files"
                                    ? "Example: uploads/company.zip"
                                    : sourceType === "folder"
                                        ? "Example: D:\\Docs\\Company"
                                        : sourceType === "website"
                                            ? "Example: https://docs.yourcompany.com"
                                            : "Example: project:my-book-universe"
                            }
                        />
                    </label>

                    <label className="label">
                        <span className="labelText">Name (optional)</span>
                        <input
                            className="input"
                            value={indexName}
                            onChange={(e) => setIndexName(e.target.value)}
                            placeholder="trusted-sources"
                        />
                    </label>

                    <div className="studioActions right">
                        <button className="btn ghost" onClick={onBack}>Back</button>
                        <button
                            className="btn"
                            onClick={() => setStep(2)}
                            disabled={!sourceValue.trim()}
                        >
                            Continue
                        </button>
                    </div>
                </div>
            )}

            {step === 2 && (
                <div className="studioForm">
                    <div className="labelText">Step 2 — Where should it be used?</div>

                    <div className="scopeGrid">
                        <label className="check">
                            <input type="checkbox" checked={scope.answer} onChange={() => toggleScope("answer")} />
                            <span>Answer questions</span>
                        </label>
                        <label className="check">
                            <input type="checkbox" checked={scope.write} onChange={() => toggleScope("write")} />
                            <span>Write content</span>
                        </label>
                        <label className="check">
                            <input type="checkbox" checked={scope.generate} onChange={() => toggleScope("generate")} />
                            <span>Generate code</span>
                        </label>
                        <label className="check">
                            <input type="checkbox" checked={scope.train} onChange={() => toggleScope("train")} />
                            <span>Train models</span>
                        </label>
                    </div>

                    <div className="tsHint">
                        Trusted Sources are always used by default — this only sets preferences for *where* it helps most.
                    </div>

                    <div className="studioActions right">
                        <button className="btn ghost" onClick={() => setStep(1)}>Back</button>
                        <button className="btn" onClick={submit} disabled={running}>
                            {running ? "Connecting…" : "Connect"}
                        </button>
                    </div>
                </div>
            )}

            {step === 3 && (
                <div className="studioForm">
                    <div className="processingTitle">✅ Your knowledge is connected.</div>
                    <div className="muted">
                        Your AI will use these trusted sources automatically to stay grounded and consistent.
                    </div>

                    <div className="studioActions right" style={{ marginTop: 12 }}>
                        <button className="btn" onClick={onDone}>Done</button>
                    </div>
                </div>
            )}
        </section>
    );
}
