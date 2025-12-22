import { useState } from "react";
import { StudioHero } from "./StudioHero";
import { StudioCard } from "./StudioCard";
import { ViewTrain } from "./ViewTrain";
import { ViewKnowledge } from "./ViewKnowledge";
import { ViewIntegrate } from "./ViewIntegrate";
import { ViewCreate } from "./ViewCreate";

export function Studio() {
    // view: 'landing' | 'train' | 'knowledge' | 'integrate' | 'create'
    const [view, setView] = useState("landing");

    if (view === "train") return <ViewTrain onBack={() => setView("landing")} />;
    if (view === "knowledge") return <ViewKnowledge onBack={() => setView("landing")} />;
    if (view === "integrate") return <ViewIntegrate onBack={() => setView("landing")} />;
    if (view === "create") return <ViewCreate onBack={() => setView("landing")} />;

    return (
        <div className="studio">
            <header className="header glass">
                <div className="titleRow">
                    <div>
                        <div className="title">AI Studio</div>
                        <div className="subtitle">
                            Deployment & Creation Center
                        </div>
                    </div>
                </div>
            </header>

            <main className="main">
                <StudioHero onNavigate={setView} />

                <section className="card glass mt">
                    <div className="cardHeader">
                        <div className="cardTitle">Your Journeys</div>
                    </div>
                    <div className="studioGrid">
                        <StudioCard
                            icon="🚀"
                            title="Train a Model"
                            desc="Fine-tune LLMs or train LoRAs on custom data."
                            onClick={() => setView("train")}
                        />
                        <StudioCard
                            icon="📚"
                            title="Connect Knowledge"
                            desc="Index documents for RAG agents."
                            onClick={() => setView("knowledge")}
                        />
                        <StudioCard
                            icon="🔌"
                            title="Integrate"
                            desc="VS Code ext, SDKs, Local Runner."
                            onClick={() => setView("integrate")}
                        />
                        <StudioCard
                            icon="✍️"
                            title="Create Content"
                            desc="Text, Images, Assistants (PC Helper)."
                            onClick={() => setView("create")}
                        />
                    </div>
                </section>
            </main>
        </div>
    );
}
