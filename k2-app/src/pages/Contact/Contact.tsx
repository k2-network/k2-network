import "./Contact.css";

export function ContactPage() {
    return (
        <div className="contact-content">
            <div className="placeholder-container">
                <div className="placeholder-icon">
                    <svg width="64" height="64" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z" />
                    </svg>
                </div>
                <h2 className="placeholder-title">Contacts</h2>
                <p className="placeholder-desc">
                    Manage your contacts and trusted trading partners.
                    <br />
                    This feature is coming soon.
                </p>
            </div>
        </div>
    );
}
