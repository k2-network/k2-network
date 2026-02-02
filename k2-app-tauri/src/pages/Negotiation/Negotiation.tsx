import "./Negotiation.css";

export function NegotiationPage() {
    return (
        <div className="negotiation-content">
            <div className="placeholder-container">
                <div className="placeholder-icon">
                    <svg width="64" height="64" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H6l-2 2V4h16v12z" />
                    </svg>
                </div>
                <h2 className="placeholder-title">Negotiation</h2>
                <p className="placeholder-desc">
                    Manage your deals and negotiations with other traders.
                    <br />
                    This feature is coming soon.
                </p>
            </div>
        </div>
    );
}
