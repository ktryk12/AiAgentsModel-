export function ViewIntegrate({ onBack }) {
    return (
        <div className="wizard glass">
            <div className="wizardHeader">
                <button className="btn ghost" onClick={onBack}>← Back</button>
                <div className="wizardTitle">Integrate</div>
            </div>
            <div className="wizardBody">
                <div className="cardGrid">
                    <div className="selCard">
                        <div className="icon">💻</div>
                        <h3>VS Code Extension</h3>
                        <p>Use your models directly in your editor.</p>
                        <button className="btn small mt">Download .vsix</button>
                    </div>
                    <div className="selCard">
                        <div className="icon">📦</div>
                        <h3>C# SDK</h3>
                        <p>Generated client for your .NET apps.</p>
                        <button className="btn small mt">Generate & Download</button>
                    </div>
                    <div className="selCard">
                        <div className="icon">🏃</div>
                        <h3>Local Runner</h3>
                        <p>Service for executing agent actions on this PC.</p>
                        <button className="btn small mt">Download Installer</button>
                    </div>
                </div>
            </div>
        </div>
    );
}
