export function StudioHero({ onNavigate }) {
    return (
        <section className="hero glass">
            <div className="heroContent">
                <h1 className="heroTitle">Build & Run Your Own AI — with Your Data</h1>
                <p className="heroSub">
                    Train custom models (LLM/LoRA), connect your knowledge base, and deploy directly into VS Code or your apps.
                </p>
                <div className="heroActions">
                    <button className="btn primary big" onClick={() => onNavigate("train")}>
                        Train a Model
                    </button>
                    <button className="btn secondary big" onClick={() => onNavigate("knowledge")}>
                        Connect Knowledge
                    </button>
                </div>
            </div>
        </section>
    );
}
