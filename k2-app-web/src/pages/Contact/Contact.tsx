import { useState, useEffect, useCallback } from "react";
import { listContacts, addContact as apiAddContact, removeContact as apiRemoveContact, updateContactNickname, pingContact as apiPingContact } from "../../api";
import "./Contact.css";

interface Contact {
    node_id: string;
    nickname: string;
    added_at: number;
    notes?: string;
}

type PingStatus = "idle" | "pinging" | "online" | "offline";

export function ContactPage() {
    const [contacts, setContacts] = useState<Contact[]>([]);
    const [loading, setLoading] = useState(true);
    const [showAddForm, setShowAddForm] = useState(false);
    const [addNodeId, setAddNodeId] = useState("");
    const [addNickname, setAddNickname] = useState("");
    const [addNotes, setAddNotes] = useState("");
    const [addError, setAddError] = useState("");
    const [addLoading, setAddLoading] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [editNickname, setEditNickname] = useState("");
    const [pingStatuses, setPingStatuses] = useState<Record<string, PingStatus>>({});
    const [copiedId, setCopiedId] = useState<string | null>(null);

    const loadContacts = useCallback(async () => {
        try {
            const list = await listContacts();
            setContacts(list.sort((a, b) => b.added_at - a.added_at));
        } catch (err) {
            console.error("Failed to load contacts:", err);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        loadContacts();
    }, [loadContacts]);

    const handleAdd = async () => {
        setAddError("");
        const trimId = addNodeId.trim();
        const trimNick = addNickname.trim();
        if (!trimId) { setAddError("Node ID is required"); return; }
        if (trimId.length !== 64) { setAddError("Node ID must be 64 hex characters"); return; }
        if (!trimNick) { setAddError("Nickname is required"); return; }

        setAddLoading(true);
        try {
            await apiAddContact(trimId, trimNick, addNotes.trim() || undefined);
            setAddNodeId("");
            setAddNickname("");
            setAddNotes("");
            setShowAddForm(false);
            await loadContacts();
        } catch (err) {
            setAddError(String(err));
        } finally {
            setAddLoading(false);
        }
    };

    const handleRemove = async (nodeId: string) => {
        try {
            await apiRemoveContact(nodeId);
            await loadContacts();
        } catch (err) {
            console.error("Failed to remove contact:", err);
        }
    };

    const handleEditSave = async (nodeId: string) => {
        const trimmed = editNickname.trim();
        if (!trimmed) { setEditingId(null); return; }
        try {
            await updateContactNickname(nodeId, trimmed);
            setEditingId(null);
            await loadContacts();
        } catch (err) {
            console.error("Failed to update nickname:", err);
        }
    };

    const handlePing = async (nodeId: string) => {
        setPingStatuses(prev => ({ ...prev, [nodeId]: "pinging" }));
        try {
            const online = await apiPingContact(nodeId);
            setPingStatuses(prev => ({ ...prev, [nodeId]: online ? "online" : "offline" }));
            setTimeout(() => setPingStatuses(prev => ({ ...prev, [nodeId]: "idle" })), 3000);
        } catch {
            setPingStatuses(prev => ({ ...prev, [nodeId]: "offline" }));
            setTimeout(() => setPingStatuses(prev => ({ ...prev, [nodeId]: "idle" })), 3000);
        }
    };

    const handleCopy = (nodeId: string) => {
        navigator.clipboard.writeText(nodeId);
        setCopiedId(nodeId);
        setTimeout(() => setCopiedId(null), 2000);
    };

    const formatDate = (ts: number) => {
        return new Date(ts * 1000).toLocaleDateString("vi-VN", {
            day: "2-digit", month: "2-digit", year: "numeric"
        });
    };

    if (loading) {
        return (
            <div className="contact-content">
                <div className="contact-loading">Loading contacts...</div>
            </div>
        );
    }

    return (
        <div className="contact-content contact-full">
            {/* Header */}
            <div className="contact-header">
                <div className="contact-header-left">
                    <h2 className="contact-title">Contacts</h2>
                    <span className="contact-count">{contacts.length}</span>
                </div>
                <button className="contact-add-btn" onClick={() => { setShowAddForm(true); setAddError(""); }}>
                    + Add Contact
                </button>
            </div>

            {/* Add Contact Form */}
            {showAddForm && (
                <div className="contact-add-form">
                    <div className="form-row">
                        <input
                            className="contact-input"
                            placeholder="Node ID (64 hex characters)"
                            value={addNodeId}
                            onChange={e => setAddNodeId(e.target.value)}
                            spellCheck={false}
                        />
                    </div>
                    <div className="form-row form-row-2col">
                        <input
                            className="contact-input"
                            placeholder="Nickname"
                            value={addNickname}
                            onChange={e => setAddNickname(e.target.value)}
                        />
                        <input
                            className="contact-input"
                            placeholder="Notes (optional)"
                            value={addNotes}
                            onChange={e => setAddNotes(e.target.value)}
                        />
                    </div>
                    {addError && <div className="contact-error">{addError}</div>}
                    <div className="form-actions">
                        <button className="btn-primary" onClick={handleAdd} disabled={addLoading}>
                            {addLoading ? "Adding..." : "Add"}
                        </button>
                        <button className="btn-secondary" onClick={() => { setShowAddForm(false); setAddError(""); }}>
                            Cancel
                        </button>
                    </div>
                </div>
            )}

            {/* Contact List */}
            {contacts.length === 0 ? (
                <div className="contact-empty">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="currentColor" opacity="0.3">
                        <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z" />
                    </svg>
                    <p>No contacts yet. Add your first contact to start trading.</p>
                </div>
            ) : (
                <div className="contact-list">
                    {contacts.map(contact => {
                        const ping = pingStatuses[contact.node_id] || "idle";
                        const shortId = `${contact.node_id.slice(0, 8)}...${contact.node_id.slice(-6)}`;
                        const isEditing = editingId === contact.node_id;

                        return (
                            <div key={contact.node_id} className="contact-card">
                                {/* Avatar */}
                                <div className="contact-avatar">
                                    {contact.nickname.charAt(0).toUpperCase()}
                                </div>

                                {/* Info */}
                                <div className="contact-info">
                                    {isEditing ? (
                                        <input
                                            className="contact-edit-input"
                                            value={editNickname}
                                            autoFocus
                                            onChange={e => setEditNickname(e.target.value)}
                                            onBlur={() => handleEditSave(contact.node_id)}
                                            onKeyDown={e => {
                                                if (e.key === "Enter") handleEditSave(contact.node_id);
                                                if (e.key === "Escape") setEditingId(null);
                                            }}
                                        />
                                    ) : (
                                        <span
                                            className="contact-nickname"
                                            onClick={() => { setEditingId(contact.node_id); setEditNickname(contact.nickname); }}
                                            title="Click to edit"
                                        >
                                            {contact.nickname}
                                        </span>
                                    )}
                                    <div className="contact-meta">
                                        <span className="contact-nodeid">{shortId}</span>
                                        <span className="contact-date">Added {formatDate(contact.added_at)}</span>
                                        {contact.notes && <span className="contact-notes">{contact.notes}</span>}
                                    </div>
                                </div>

                                {/* Ping badge */}
                                <div className={`ping-badge ping-${ping}`}>
                                    {ping === "pinging" && <span className="ping-dot pulsing" />}
                                    {ping === "online" && <span className="ping-dot online" />}
                                    {ping === "offline" && <span className="ping-dot offline" />}
                                    {ping === "idle" && <span className="ping-dot idle" />}
                                    <span className="ping-label">
                                        {ping === "pinging" ? "Pinging..." : ping === "idle" ? "" : ping}
                                    </span>
                                </div>

                                {/* Actions */}
                                <div className="contact-actions">
                                    <button
                                        className="action-btn"
                                        title="Copy Node ID"
                                        onClick={() => handleCopy(contact.node_id)}
                                    >
                                        {copiedId === contact.node_id ? (
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41L9 16.17z" /></svg>
                                        ) : (
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z" /></svg>
                                        )}
                                    </button>
                                    <button
                                        className="action-btn"
                                        title="Ping"
                                        onClick={() => handlePing(contact.node_id)}
                                        disabled={ping === "pinging"}
                                    >
                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 14.5v-9l6 4.5-6 4.5z" /></svg>
                                    </button>
                                    <button
                                        className="action-btn action-btn-danger"
                                        title="Remove"
                                        onClick={() => handleRemove(contact.node_id)}
                                    >
                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z" /></svg>
                                    </button>
                                </div>
                            </div>
                        );
                    })}
                </div>
            )}
        </div>
    );
}
