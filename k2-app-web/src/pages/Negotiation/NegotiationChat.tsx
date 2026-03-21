/**
 * NegotiationChat - Discord-style Chat Interface
 * 
 * Left: Contact list (DM-style sidebar)
 * Right: Chat interface with bubble messages
 * 
 * Uses Tauri commands for contact management
 * P2P messaging via iroh-gossip (temporary, no persistence)
 */
import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
    listContacts,
    getMyNodeId,
    addContact as apiAddContact,
    removeContact as apiRemoveContact,
    pingContact,
    sendP2pMessage as apiSendP2pMessage,
    k2ws,
} from '../../api';
import { useAuth } from '../../context/AuthContext';
import { IoPersonAdd, IoSearch, IoEllipsisVertical, IoClose, IoCheckmark, IoChevronDown, IoChevronUp } from 'react-icons/io5';
import type { Contact } from '../../types';
import './NegotiationChat.css';

// Message type for P2P chat
interface ChatMessage {
    id: string;
    senderId: string;
    senderName: string;
    content: string;
    timestamp: number;
    isMe: boolean;
}

// Contact with online status
interface ContactWithStatus extends Contact {
    isOnline?: boolean;
    unreadCount?: number;
    lastMessage?: string;
}

// Generate avatar color from name/id
const getAvatarColor = (str: string): string => {
    const colors = [
        '#F15CDD', '#47E069', '#4DA6FF', '#FFB84D',
        '#FF6B6B', '#9B59B6', '#1ABC9C', '#F39C12',
        '#E74C3C', '#3498DB', '#2ECC71', '#E91E63',
    ];
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
        hash = str.charCodeAt(i) + ((hash << 5) - hash);
    }
    return colors[Math.abs(hash) % colors.length];
};

// Get initials from nickname
const getInitials = (name: string): string => {
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) {
        return (parts[0][0] + parts[1][0]).toUpperCase();
    }
    return name.substring(0, 2).toUpperCase();
};

// Format timestamp
const formatTime = (timestamp: number): string => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffDays = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24));

    if (diffDays === 0) {
        return date.toLocaleTimeString('vi-VN', { hour: '2-digit', minute: '2-digit' });
    } else if (diffDays === 1) {
        return 'Hôm qua';
    } else if (diffDays < 7) {
        return date.toLocaleDateString('vi-VN', { weekday: 'short' });
    }
    return date.toLocaleDateString('vi-VN', { day: '2-digit', month: '2-digit' });
};

interface DealInfo {
    title: string;
    priceMin: number;
    priceMax: number;
    currency: string;
}

interface NegotiationChatProps {
    openChatWith?: { nodeId: string; name: string; deal?: DealInfo } | null;
    onChatOpened?: () => void;
}

export const NegotiationChat: React.FC<NegotiationChatProps> = ({ openChatWith, onChatOpened }) => {
    const { sessionId, user } = useAuth();
    // State
    const [contacts, setContacts] = useState<ContactWithStatus[]>([]);
    const [selectedContact, setSelectedContact] = useState<ContactWithStatus | null>(null);
    const [messages, setMessages] = useState<Map<string, ChatMessage[]>>(new Map());
    const [inputMessage, setInputMessage] = useState('');
    const [searchQuery, setSearchQuery] = useState('');
    const [showAddContact, setShowAddContact] = useState(false);
    const [newContactId, setNewContactId] = useState('');
    const [newContactNickname, setNewContactNickname] = useState('');
    const [myNodeId, setMyNodeId] = useState('');
    const [showContactMenu, setShowContactMenu] = useState(false);
    const [dealPanelExpanded, setDealPanelExpanded] = useState(true);
    const [activeDeal, setActiveDeal] = useState<DealInfo | null>(null);

    // Refs
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);
    const contactsRef = useRef<ContactWithStatus[]>([]);
    const selectedContactRef = useRef<ContactWithStatus | null>(null);

    // Keep refs updated
    useEffect(() => {
        contactsRef.current = contacts;
    }, [contacts]);

    useEffect(() => {
        selectedContactRef.current = selectedContact;
    }, [selectedContact]);

    // Close menu when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
                setShowContactMenu(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    // Scroll to bottom
    const scrollToBottom = useCallback(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, []);

    // Load contacts on mount
    useEffect(() => {
        const loadContacts = async () => {
            try {
                const contactList = await listContacts();
                setContacts(contactList.map(c => ({ ...c, notes: c.notes ?? null, isOnline: false, unreadCount: 0 })));

                // Get my node id
                const nodeId = await getMyNodeId();
                setMyNodeId(nodeId);
            } catch (err) {
                console.error('Failed to load contacts:', err);
            }
        };
        loadContacts();
    }, []);

    // Auto-open chat with a specific contact when navigated from NegotiationDashboard
    useEffect(() => {
        if (!openChatWith) return;

        const autoOpen = async () => {
            // Lưu deal info để hiển thị trong deal panel
            if (openChatWith.deal) setActiveDeal(openChatWith.deal);

            // Kiểm tra đã có trong contacts chưa
            let contact = contactsRef.current.find(c => c.node_id === openChatWith.nodeId);

            if (!contact) {
                // Tự động thêm contact
                try {
                    const added = await apiAddContact(openChatWith.nodeId, openChatWith.name, 'Added from negotiation');
                    contact = { ...added, notes: added.notes ?? null, isOnline: false, unreadCount: 0 };
                    setContacts(prev => [contact!, ...prev]);
                } catch (err) {
                    console.error('[NegotiationChat] Failed to add contact:', err);
                    return;
                }
            }

            await handleSelectContact(contact);
            onChatOpened?.();
        };

        autoOpen();
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [openChatWith]);

    // Listen for incoming P2P messages
    useEffect(() => {
        const setupListener = () => {
            const unlisten = k2ws.listen('k2://chat-message', async (event) => {
                const payload = event as any;
                console.log('[NegotiationChat] Received message:', payload);

                // Skip messages from self (already added locally when sending)
                // sender_node_id is the sessionId of the sender
                if (payload.sender_node_id === sessionId) {
                    console.log('[NegotiationChat] Skipping own message');
                    return;
                }

                // Check if sender is already in contacts
                const existingContact = contactsRef.current.find(c => c.node_id === payload.sender_node_id);

                if (!existingContact) {
                    console.log('[NegotiationChat] Auto-adding unknown sender as contact:', payload.sender_name);
                    try {
                        // Dùng shortId làm nickname để tránh trùng "Guest"
                        const shortId = payload.sender_node_id.slice(0, 8);
                        const senderName = `Peer ${shortId}`;
                        const newContact = await apiAddContact(
                            payload.sender_node_id,
                            senderName,
                            'Auto-added from incoming message'
                        );

                        // Add to contacts list with unread count = 1 for the incoming message
                        setContacts(prev => [...prev, {
                            ...newContact,
                            notes: newContact.notes ?? null,
                            isOnline: true, // They just sent a message, so they're online
                            unreadCount: 1, // Mark as having 1 unread message
                            lastMessage: payload.content.substring(0, 50) + (payload.content.length > 50 ? '...' : '')
                        }]);

                        console.log('[NegotiationChat] Successfully auto-added contact:', senderName);

                        // Show a brief notification that a new contact was added (with permission check)
                        if ('Notification' in window) {
                            if (Notification.permission === 'granted') {
                                const notification = new Notification('Tin nhắn mới', {
                                    body: `${senderName} đã gửi tin nhắn cho bạn`,
                                    icon: '/favicon.ico' // Optional: add an icon
                                });

                                // Auto-close notification after 3 seconds
                                setTimeout(() => notification.close(), 3000);
                            } else if (Notification.permission !== 'denied') {
                                // Request permission if not denied
                                Notification.requestPermission().then(permission => {
                                    if (permission === 'granted') {
                                        const notification = new Notification('Tin nhắn mới', {
                                            body: `${senderName} đã gửi tin nhắn cho bạn`,
                                            icon: '/favicon.ico'
                                        });
                                        setTimeout(() => notification.close(), 3000);
                                    }
                                });
                            }
                        }

                    } catch (err) {
                        console.error('[NegotiationChat] Failed to auto-add contact:', err);
                        // Continue processing message even if contact addition fails
                    }
                } else {
                    // Update unread count for existing contact if not currently selected
                    if (selectedContactRef.current?.node_id !== payload.sender_node_id) {
                        setContacts(prev => prev.map(c => {
                            if (c.node_id === payload.sender_node_id) {
                                return {
                                    ...c,
                                    unreadCount: (c.unreadCount || 0) + 1,
                                    lastMessage: payload.content.substring(0, 50) + (payload.content.length > 50 ? '...' : '')
                                };
                            }
                            return c;
                        }));
                    }
                }

                const msg: ChatMessage = {
                    id: `${payload.timestamp || Date.now()}-${payload.sender_node_id}`,
                    senderId: payload.sender_node_id,
                    senderName: payload.sender_name || 'Unknown',
                    content: payload.content,
                    timestamp: payload.timestamp || Date.now(),
                    isMe: false,
                };

                setMessages(prev => {
                    const newMap = new Map(prev);
                    const existing = newMap.get(payload.sender_node_id) || [];

                    // Check for duplicate (same timestamp + sender)
                    const isDuplicate = existing.some(m => m.id === msg.id);
                    if (isDuplicate) {
                        console.log('[NegotiationChat] Skipping duplicate message');
                        return prev;
                    }

                    newMap.set(payload.sender_node_id, [...existing, msg]);
                    return newMap;
                });
            });

            return unlisten;
        };

        const unlisten = setupListener();
        return () => {
            unlisten();
        };
    }, [sessionId]); // chỉ re-register khi sessionId đổi (không bao giờ), không re-register khi chọn contact

    // Scroll when messages change
    useEffect(() => {
        scrollToBottom();
    }, [messages, selectedContact, scrollToBottom]);

    // Ping contacts to check online status
    useEffect(() => {
        const checkOnlineStatus = async () => {
            for (const contact of contacts) {
                try {
                    const isOnline = await pingContact(contact.node_id);
                    setContacts(prev => prev.map(c =>
                        c.node_id === contact.node_id ? { ...c, isOnline } : c
                    ));
                } catch {
                    // Ignore errors
                }
            }
        };

        // Check every 30 seconds
        const interval = setInterval(checkOnlineStatus, 30000);
        checkOnlineStatus(); // Initial check

        return () => clearInterval(interval);
    }, [contacts.length]);

    // Filter contacts by search
    const filteredContacts = contacts.filter(c =>
        c.nickname.toLowerCase().includes(searchQuery.toLowerCase()) ||
        c.node_id.toLowerCase().includes(searchQuery.toLowerCase())
    );

    // Select contact
    const handleSelectContact = async (contact: ContactWithStatus) => {
        setSelectedContact(contact);
        // Clear unread count
        setContacts(prev => prev.map(c =>
            c.node_id === contact.node_id ? { ...c, unreadCount: 0 } : c
        ));

            inputRef.current?.focus();
    };

    // Send message
    const handleSendMessage = async () => {
        if (!inputMessage.trim() || !selectedContact) return;

        const timestamp = Date.now();
        // ID nhất quán với incoming message format: timestamp-senderId
        const msgId = `${timestamp}-${sessionId}`;
        const content = inputMessage.trim();

        const msg: ChatMessage = {
            id: msgId,
            senderId: myNodeId,
            senderName: 'Me',
            content,
            timestamp,
            isMe: true,
        };

        // Add to local messages trước
        setMessages(prev => {
            const newMap = new Map(prev);
            const existing = newMap.get(selectedContact.node_id) || [];
            // Guard chống double-click
            if (existing.some(m => m.id === msgId)) return prev;
            newMap.set(selectedContact.node_id, [...existing, msg]);
            return newMap;
        });

        setInputMessage('');

        // Gửi P2P — không fallback relay để tránh duplicate
        try {
            await apiSendP2pMessage(
                selectedContact.node_id,
                content,
                sessionId,
                user?.username ?? 'Guest',
            );
        } catch (err) {
            console.error('P2P send failed:', err);
        }
    };

    // Add new contact
    const handleAddContact = async () => {
        if (!newContactId.trim() || !newContactNickname.trim()) return;

        try {
            const contact = await apiAddContact(newContactId.trim(), newContactNickname.trim());
            setContacts(prev => [...prev, { ...contact, notes: contact.notes ?? null, isOnline: false, unreadCount: 0 }]);
            setShowAddContact(false);
            setNewContactId('');
            setNewContactNickname('');
        } catch (err) {
            console.error('Failed to add contact:', err);
        }
    };

    // Delete contact
    const handleDeleteContact = async () => {
        if (!selectedContact) return;

        const confirmDelete = window.confirm(`Bạn có chắc muốn xóa liên hệ "${selectedContact.nickname}"?`);
        if (!confirmDelete) return;

        try {
            await apiRemoveContact(selectedContact.node_id);
            setContacts(prev => prev.filter(c => c.node_id !== selectedContact.node_id));
            // Also clear messages for this contact
            setMessages(prev => {
                const newMap = new Map(prev);
                newMap.delete(selectedContact.node_id);
                return newMap;
            });
            setSelectedContact(null);
            setShowContactMenu(false);
            console.log('[NegotiationChat] Deleted contact:', selectedContact.nickname);
        } catch (err) {
            console.error('Failed to delete contact:', err);
        }
    };

    // Clear chat history
    const handleClearChat = () => {
        if (!selectedContact) return;

        const confirmClear = window.confirm(`Xóa toàn bộ tin nhắn với "${selectedContact.nickname}"?`);
        if (!confirmClear) return;

        setMessages(prev => {
            const newMap = new Map(prev);
            newMap.delete(selectedContact.node_id);
            return newMap;
        });
        setShowContactMenu(false);
        console.log('[NegotiationChat] Cleared chat with:', selectedContact.nickname);
    };

    // Get messages for selected contact
    const currentMessages = selectedContact ? (messages.get(selectedContact.node_id) || []) : [];

    return (
        <div className="negotiation-chat">
            {/* Left Sidebar - Contact List (Discord-style) */}
            <div className="chat-sidebar">
                <div className="sidebar-header">
                    <h3>Direct Messages</h3>
                    <button
                        className="add-contact-btn"
                        onClick={() => setShowAddContact(true)}
                        title="Thêm liên hệ"
                    >
                        <IoPersonAdd />
                    </button>
                </div>

                {/* Search */}
                <div className="sidebar-search">
                    <IoSearch className="search-icon" />
                    <input
                        type="text"
                        placeholder="Tìm kiếm..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                    />
                </div>

                {/* Contact List */}
                <div className="contact-list">
                    {filteredContacts.length === 0 ? (
                        <div className="no-contacts">
                            <p>Chưa có liên hệ nào</p>
                            <button onClick={() => setShowAddContact(true)}>
                                Thêm liên hệ
                            </button>
                        </div>
                    ) : (
                        filteredContacts.map(contact => (
                            <div
                                key={contact.node_id}
                                className={`contact-item ${selectedContact?.node_id === contact.node_id ? 'selected' : ''}`}
                                onClick={() => handleSelectContact(contact)}
                            >
                                <div className="contact-avatar" style={{ backgroundColor: getAvatarColor(contact.nickname) }}>
                                    {getInitials(contact.nickname)}
                                    <span className={`online-indicator ${contact.isOnline ? 'online' : ''}`} />
                                </div>
                                <div className="contact-info">
                                    <div className="contact-name-row">
                                        <span className="contact-name">{contact.nickname}</span>
                                        {contact.notes === 'Auto-added from incoming message' && (
                                            <span className="auto-added-badge">Auto</span>
                                        )}
                                    </div>
                                    {contact.lastMessage && (
                                        <span className="contact-preview">{contact.lastMessage}</span>
                                    )}
                                </div>
                                {contact.unreadCount ? (
                                    <span className="unread-badge">{contact.unreadCount}</span>
                                ) : null}
                            </div>
                        ))
                    )}
                </div>
            </div>

            {/* Right Side - Chat Area */}
            <div className="chat-main">
                {selectedContact ? (
                    <>
                        {/* Chat Header */}
                        <div className="chat-area-header">
                            <div className="chat-contact-info">
                                <div
                                    className="contact-avatar"
                                    style={{ backgroundColor: getAvatarColor(selectedContact.nickname) }}
                                >
                                    {getInitials(selectedContact.nickname)}
                                </div>
                                <div className="contact-details">
                                    <span className="contact-name">{selectedContact.nickname}</span>
                                    <span className={`contact-status ${selectedContact.isOnline ? 'online' : ''}`}>
                                        {selectedContact.isOnline ? 'Đang hoạt động' : 'Offline'}
                                    </span>
                                </div>
                            </div>
                            <button className="menu-btn" onClick={() => setShowContactMenu(!showContactMenu)}>
                                <IoEllipsisVertical />
                            </button>
                            {/* Contact Menu Dropdown */}
                            {showContactMenu && (
                                <div className="contact-menu-dropdown" ref={menuRef}>
                                    <button className="menu-item" onClick={handleClearChat}>
                                        Xóa tin nhắn
                                    </button>
                                    <button className="menu-item danger" onClick={handleDeleteContact}>
                                        Xóa liên hệ
                                    </button>
                                </div>
                            )}
                        </div>

                        {/* Deal Info Panel */}
                        {activeDeal && (
                        <div className={`deal-info-panel ${dealPanelExpanded ? 'expanded' : 'collapsed'}`}>
                            {dealPanelExpanded ? (
                                <>
                                    <div className="deal-header">
                                        <div className="deal-header-left">
                                            <span className="deal-label">DEAL</span>
                                            <span className="deal-status pending">Đang đàm phán</span>
                                        </div>
                                        <button className="deal-toggle-btn" onClick={() => setDealPanelExpanded(false)}>
                                            <IoChevronUp />
                                        </button>
                                    </div>
                                    <div className="deal-content">
                                        <div className="deal-title">{activeDeal.title}</div>
                                        <div className="deal-details">
                                            <div className="deal-row">
                                                <span className="deal-key">Giá đề xuất:</span>
                                                <span className="deal-value price">
                                                    {activeDeal.priceMin.toLocaleString()} – {activeDeal.priceMax.toLocaleString()} {activeDeal.currency}
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </>
                            ) : (
                                <div className="deal-collapsed-row">
                                    <span className="deal-label">DEAL</span>
                                    <span className="deal-title-compact">{activeDeal.title}</span>
                                    <span className="deal-separator">•</span>
                                    <span className="deal-value price">
                                        {activeDeal.priceMin.toLocaleString()} {activeDeal.currency}
                                    </span>
                                    <span className="deal-separator">•</span>
                                    <span className="deal-status pending">Đang đàm phán</span>
                                    <button className="deal-toggle-btn" onClick={() => setDealPanelExpanded(true)}>
                                        <IoChevronDown />
                                    </button>
                                </div>
                            )}
                        </div>
                        )}

                        {/* Messages */}
                        <div className="chat-messages-area">
                            {currentMessages.length === 0 ? (
                                <div className="no-messages">
                                    <div
                                        className="empty-avatar"
                                        style={{ backgroundColor: getAvatarColor(selectedContact.nickname) }}
                                    >
                                        {getInitials(selectedContact.nickname)}
                                    </div>
                                    <h3>{selectedContact.nickname}</h3>
                                    <p>Bắt đầu cuộc trò chuyện với {selectedContact.nickname}</p>
                                </div>
                            ) : (
                                currentMessages.map(msg => (
                                    <div
                                        key={msg.id}
                                        className={`chat-message ${msg.isMe ? 'me' : 'them'}`}
                                    >
                                        {!msg.isMe && (
                                            <div
                                                className="message-avatar"
                                                style={{ backgroundColor: getAvatarColor(selectedContact.nickname) }}
                                            >
                                                {getInitials(selectedContact.nickname)}
                                            </div>
                                        )}
                                        <div className="message-bubble">
                                            <span className="message-content">{msg.content}</span>
                                            <span className="message-time">{formatTime(msg.timestamp)}</span>
                                        </div>
                                    </div>
                                ))
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        {/* Input Area */}
                        <div className="chat-input-area">
                            <input
                                ref={inputRef}
                                type="text"
                                placeholder={`Nhắn tin cho ${selectedContact.nickname}...`}
                                value={inputMessage}
                                onChange={(e) => setInputMessage(e.target.value)}
                                onKeyDown={(e) => e.key === 'Enter' && handleSendMessage()}
                            />
                        </div>
                    </>
                ) : (
                    <div className="no-chat-selected">
                        <div className="empty-state">
                            <svg width="80" height="80" viewBox="0 0 24 24" fill="currentColor" opacity="0.3">
                                <path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H6l-2 2V4h16v12z" />
                            </svg>
                            <h3>Chọn một cuộc trò chuyện</h3>
                            <p>Chọn liên hệ từ danh sách bên trái để bắt đầu đàm phán</p>
                        </div>
                    </div>
                )}
            </div>

            {/* Add Contact Modal */}
            {showAddContact && (
                <div className="modal-overlay" onClick={() => setShowAddContact(false)}>
                    <div className="add-contact-modal" onClick={e => e.stopPropagation()}>
                        <div className="modal-header">
                            <h3>Thêm liên hệ mới</h3>
                            <button className="close-btn" onClick={() => setShowAddContact(false)}>
                                <IoClose />
                            </button>
                        </div>
                        <div className="modal-body">
                            <div className="form-group">
                                <label>Node ID</label>
                                <input
                                    type="text"
                                    placeholder="Nhập Node ID của đối tác..."
                                    value={newContactId}
                                    onChange={(e) => setNewContactId(e.target.value)}
                                />
                            </div>
                            <div className="form-group">
                                <label>Tên hiển thị</label>
                                <input
                                    type="text"
                                    placeholder="Nhập tên cho liên hệ..."
                                    value={newContactNickname}
                                    onChange={(e) => setNewContactNickname(e.target.value)}
                                />
                            </div>
                        </div>
                        <div className="modal-footer">
                            <button className="cancel-btn" onClick={() => setShowAddContact(false)}>
                                Hủy
                            </button>
                            <button
                                className="confirm-btn"
                                onClick={handleAddContact}
                                disabled={!newContactId.trim() || !newContactNickname.trim()}
                            >
                                <IoCheckmark /> Thêm
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};

export default NegotiationChat;
