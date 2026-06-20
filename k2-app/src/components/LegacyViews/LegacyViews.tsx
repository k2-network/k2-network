// Legacy Views from original App.tsx
// These can be reused in the new structure if needed

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { scan, Format, checkPermissions, requestPermissions } from "@tauri-apps/plugin-barcode-scanner";
import { readFile } from "@tauri-apps/plugin-fs";

interface Contact {
    node_id: string;
    nickname: string;
    added_at: number;
    notes: string | null;
}

// ============ CONTACTS VIEW ============
export function ContactsView() {
    const [myNodeId, setMyNodeId] = useState("");
    const [showMyQR, setShowMyQR] = useState(false);
    const [myQrSvg, setMyQrSvg] = useState("");
    const [contacts, setContacts] = useState<Contact[]>([]);
    const [newNodeId, setNewNodeId] = useState("");
    const [newNickname, setNewNickname] = useState("");
    const [statusMsg, setStatusMsg] = useState("");
    const [pingStatus, setPingStatus] = useState<Record<string, string>>({});

    useEffect(() => {
        loadData();
    }, []);

    const loadData = async () => {
        try {
            const nodeId = await invoke<string>("get_my_node_id");
            setMyNodeId(nodeId);

            const contactList = await invoke<Contact[]>("list_contacts");
            setContacts(contactList);
        } catch (err) {
            setStatusMsg(`Error loading: ${err}`);
        }
    };

    const showMyIdQR = async () => {
        if (!myQrSvg && myNodeId) {
            const svg = await invoke<string>("generate_qr_svg", { data: myNodeId });
            setMyQrSvg(svg);
        }
        setShowMyQR(!showMyQR);
    };

    const addContact = async () => {
        if (!newNodeId.trim() || !newNickname.trim()) {
            setStatusMsg("Please enter NodeID and nickname");
            return;
        }

        try {
            await invoke("add_contact", {
                nodeId: newNodeId.trim(),
                nickname: newNickname.trim(),
                notes: null
            });
            setNewNodeId("");
            setNewNickname("");
            setStatusMsg("Contact added!");
            loadData();
        } catch (err) {
            setStatusMsg(`Error adding contact: ${err}`);
        }
    };

    const removeContact = async (nodeId: string) => {
        try {
            await invoke("remove_contact", { nodeId });
            setStatusMsg("Contact removed");
            loadData();
        } catch (err) {
            setStatusMsg(`Error: ${err}`);
        }
    };

    const pingContact = async (nodeId: string) => {
        setPingStatus(prev => ({ ...prev, [nodeId]: "pinging..." }));
        try {
            const online = await invoke<boolean>("ping_contact", { nodeId });
            setPingStatus(prev => ({ ...prev, [nodeId]: online ? "✅ Online" : "❌ Offline" }));
        } catch (err) {
            setPingStatus(prev => ({ ...prev, [nodeId]: "❌ Error" }));
        }
    };

    const scanContactQR = async () => {
        try {
            const permStatus = await checkPermissions();
            if (permStatus !== 'granted') {
                const newStatus = await requestPermissions();
                if (newStatus !== 'granted') {
                    setStatusMsg("Camera permission denied");
                    return;
                }
            }

            const result = await scan({
                formats: [Format.QRCode],
                windowed: false,
            });

            if (result && result.content) {
                setNewNodeId(result.content);
                setStatusMsg("NodeID scanned! Enter a nickname.");
            }
        } catch (err: any) {
            setStatusMsg(`Scan error: ${err?.message || err}`);
        }
    };

    return (
        <div className="card contacts-view">
            {/* My Node ID Section */}
            <div className="my-id-section">
                <div className="section-title">📱 My Node ID</div>
                <div className="my-id-value" onClick={() => navigator.clipboard.writeText(myNodeId)}>
                    {myNodeId ? `${myNodeId.slice(0, 20)}...` : "Loading..."}
                </div>
                <button className="small-btn" onClick={showMyIdQR}>
                    {showMyQR ? "Hide QR" : "Show QR"}
                </button>
                {showMyQR && myQrSvg && (
                    <div className="qr-container" dangerouslySetInnerHTML={{ __html: myQrSvg }} />
                )}
                <p className="hint">Chia sẻ NodeID này để bạn bè thêm vào danh bạ</p>
            </div>

            {/* Add Contact Section */}
            <div className="add-contact-section">
                <div className="section-title">➕ Add Contact</div>
                <div className="add-contact-form">
                    <input
                        placeholder="Paste NodeID or scan QR"
                        value={newNodeId}
                        onChange={(e) => setNewNodeId(e.target.value)}
                    />
                    <button className="scan-btn" onClick={scanContactQR}>📷</button>
                </div>
                <input
                    placeholder="Nickname"
                    value={newNickname}
                    onChange={(e) => setNewNickname(e.target.value)}
                />
                <button onClick={addContact}>Add Contact</button>
            </div>

            {/* Contact List */}
            <div className="contact-list-section">
                <div className="section-title">👥 Contacts ({contacts.length})</div>
                {contacts.length === 0 ? (
                    <p className="no-contacts">No contacts yet. Add one above!</p>
                ) : (
                    <div className="contact-list">
                        {contacts.map((contact) => (
                            <div key={contact.node_id} className="contact-item">
                                <div className="contact-info">
                                    <div className="contact-nickname">{contact.nickname}</div>
                                    <div className="contact-id">{contact.node_id.slice(0, 16)}...</div>
                                </div>
                                <div className="contact-actions">
                                    <span className="ping-status">{pingStatus[contact.node_id] || ""}</span>
                                    <button className="action-btn" onClick={() => pingContact(contact.node_id)}>Ping</button>
                                    <button className="action-btn danger" onClick={() => removeContact(contact.node_id)}>✕</button>
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>

            {statusMsg && <p className="status-msg">{statusMsg}</p>}
        </div>
    );
}

// ============ SEND VIEW ============
export function SendView() {
    const [manualPath, setManualPath] = useState("");
    const [pickedPath, setPickedPath] = useState<string | null>(null);
    const [fileBytes, setFileBytes] = useState<number[] | null>(null);
    const [fileName, setFileName] = useState<string | null>(null);
    const [generatedTicket, setGeneratedTicket] = useState("");
    const [qrSvg, setQrSvg] = useState("");
    const [statusMsg, setStatusMsg] = useState("");
    const [isProcessing, setIsProcessing] = useState(false);

    const pickFile = async () => {
        try {
            const selected = await open({
                multiple: false,
                title: "Select a file to share",
            });

            if (selected) {
                const pathStr = selected as string;

                if (pathStr.startsWith("content://")) {
                    setStatusMsg("Reading file from Android...");
                    try {
                        const bytes = await readFile(pathStr);
                        const byteArray = Array.from(bytes) as number[];
                        setFileBytes(byteArray);
                        const name = pathStr.split('/').pop() || 'shared_file.bin';
                        setFileName(name);
                        setPickedPath(null);
                        setGeneratedTicket("");
                        setQrSvg("");
                        setStatusMsg(`File ready: ${byteArray.length} bytes`);
                    } catch (readErr) {
                        setStatusMsg(`Error reading file: ${readErr}`);
                    }
                } else {
                    setPickedPath(pathStr);
                    setFileBytes(null);
                    setFileName(null);
                    setGeneratedTicket("");
                    setQrSvg("");
                    setStatusMsg(`File selected: ${pathStr.split(/[/\\]/).pop()}`);
                }
            }
        } catch (err) {
            setStatusMsg(`File picker error: ${err}`);
        }
    };

    const useManualPath = () => {
        if (manualPath.trim()) {
            setPickedPath(manualPath.trim());
            setGeneratedTicket("");
            setQrSvg("");
            setStatusMsg("File path set.");
        }
    };

    const shareFile = async () => {
        if (!pickedPath && !fileBytes) return;

        setIsProcessing(true);
        setStatusMsg("Generating share ticket...");

        try {
            let ticket = "";
            if (fileBytes && fileName) {
                ticket = await invoke<string>("share_bytes", {
                    bytes: fileBytes,
                    filename: fileName
                });
            } else if (pickedPath) {
                ticket = await invoke<string>("share_file", { path: pickedPath });
            }
            setGeneratedTicket(ticket);
            const svg = await invoke<string>("generate_qr_svg", { data: ticket });
            setQrSvg(svg);
            setStatusMsg("Ready to share!");
        } catch (err) {
            setStatusMsg(`Error sharing: ${err}`);
        }

        setIsProcessing(false);
    };

    const resetShare = () => {
        setGeneratedTicket("");
        setQrSvg("");
        setPickedPath(null);
        setFileBytes(null);
        setFileName(null);
        setManualPath("");
        setStatusMsg("");
    };

    if (generatedTicket) {
        return (
            <div className="card">
                <div className="qr-section">
                    <p className="qr-title">SCAN QR TO RECEIVE:</p>
                    {qrSvg && (
                        <div className="qr-container" dangerouslySetInnerHTML={{ __html: qrSvg }} />
                    )}
                    <p className="qr-hint">Quét QR → Copy text → Paste vào app nhận</p>
                    <details className="ticket-details">
                        <summary>Hoặc copy ticket thủ công</summary>
                        <div className="ticket-box">{generatedTicket}</div>
                    </details>
                </div>
                <button onClick={resetShare} className="secondary">
                    Tiếp tục chia sẻ file
                </button>
            </div>
        );
    }

    return (
        <div className="card">
            <div className="file-picker" onClick={pickFile}>
                {pickedPath || fileName ? (
                    <>
                        <div>Selected:</div>
                        <div className="file-name">
                            {fileName || pickedPath?.split(/[/\\]/).pop()}
                        </div>
                        {fileBytes && <div className="file-size">({fileBytes.length} bytes)</div>}
                    </>
                ) : (
                    "Tap here to select a file"
                )}
            </div>

            <div className="manual-path-section">
                <input
                    placeholder="Or enter file path manually"
                    value={manualPath}
                    onChange={(e) => setManualPath(e.target.value)}
                />
                <button onClick={useManualPath} className="small-btn">
                    Use Path
                </button>
            </div>

            <button
                disabled={(!pickedPath && !fileBytes) || isProcessing}
                onClick={shareFile}
            >
                {isProcessing ? "Processing..." : "Share File"}
            </button>

            {statusMsg && <p className="status-msg">{statusMsg}</p>}
        </div>
    );
}

// ============ RECEIVE VIEW ============
export function ReceiveView() {
    const [ticketInput, setTicketInput] = useState("");
    const [statusMsg, setStatusMsg] = useState("");
    const [isDownloading, setIsDownloading] = useState(false);
    const [isScanning, setIsScanning] = useState(false);

    const scanQRCode = async () => {
        setIsScanning(true);
        setStatusMsg("Checking camera permission...");

        try {
            const permStatus = await checkPermissions();
            if (permStatus !== 'granted') {
                setStatusMsg("Requesting camera permission...");
                const newStatus = await requestPermissions();
                if (newStatus !== 'granted') {
                    setStatusMsg("Camera permission denied. Please enable in settings.");
                    setIsScanning(false);
                    return;
                }
            }

            setStatusMsg("Opening camera...");
            const result = await scan({
                formats: [Format.QRCode],
                windowed: false,
            });

            if (result && result.content) {
                setTicketInput(result.content);
                setStatusMsg("QR scanned successfully!");
            }
        } catch (err: any) {
            const errMsg = err?.message || JSON.stringify(err) || 'Camera permission denied?';
            setStatusMsg(`Scan error: ${errMsg}`);
        }

        setIsScanning(false);
    };

    const downloadFile = async () => {
        if (!ticketInput.trim()) return;

        setIsDownloading(true);
        setStatusMsg("Connecting to peer...");

        try {
            setStatusMsg("Downloading...");
            const savedPath = await invoke<string>("download_file", { ticket: ticketInput.trim() });
            setStatusMsg(`SUCCESS! Saved: ${savedPath}`);
        } catch (err) {
            setStatusMsg(`Download failed: ${err}`);
        }

        setIsDownloading(false);
    };

    return (
        <div className="card">
            <div className="scan-instruction" onClick={scanQRCode}>
                <p className="scan-title">
                    {isScanning ? "Scanning..." : "Tap to Scan QR Code"}
                </p>
                <p className="scan-hint">
                    Use camera to scan QR (Android/iOS), or paste ticket below
                </p>
            </div>

            <div className="divider">
                <span>OR PASTE TICKET</span>
            </div>

            <input
                placeholder="Paste Ticket Here"
                value={ticketInput}
                onChange={(e) => setTicketInput(e.target.value)}
            />

            <button
                className="download"
                disabled={!ticketInput.trim() || isDownloading}
                onClick={downloadFile}
            >
                {isDownloading ? "Downloading..." : "Download File"}
            </button>

            {statusMsg && <p className="status-msg-receive">{statusMsg}</p>}
        </div>
    );
}
