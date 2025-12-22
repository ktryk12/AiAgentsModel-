export function StudioCard({ icon, title, desc, onClick }) {
    return (
        <button className="studioCard glass" onClick={onClick}>
            <div className="studioCardIcon">{icon}</div>
            <div className="studioCardBody">
                <div className="studioCardTitle">{title}</div>
                <div className="studioCardDesc">{desc}</div>
            </div>
        </button>
    );
}
