export function VerifiedBadge({ sourcesCount }) {
    return (
        <div className="verified">
            <span className="verifiedDot">✅</span>
            <div>
                <div className="verifiedTitle">Verified</div>
                <div className="verifiedSub">
                    {typeof sourcesCount === "number"
                        ? `Answer based on ${sourcesCount} sources`
                        : "Answer based on your trusted sources"}
                </div>
            </div>
        </div>
    );
}
