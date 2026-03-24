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
import ReactDOM from 'react-dom';
import {
    listContacts,
    addContact as apiAddContact,
    removeContact as apiRemoveContact,
    pingContact,
    sendChatMessage as apiSendChatMessage,
    getChatHistory,
    clearChatHistory,
    uploadChatFile,
    k2ws,
} from '../../api';
import { useAuth } from '../../context/AuthContext';
import { useNotifications } from '../../context/NotificationContext';
import {
    sendFriendRequest,
    getPendingRequests,
    acceptRequest as apiAcceptRequest,
    declineRequest as apiDeclineRequest,
    type FriendRequest as FriendRequestType,
} from '../../api/friendRequests';
import { IoPersonAdd, IoSearch, IoEllipsisVertical, IoClose, IoCheckmark, IoChevronDown, IoChevronUp, IoSend, IoHappyOutline, IoAttachOutline, IoReturnUpBack, IoTrashOutline, IoPersonRemoveOutline, IoDocumentOutline, IoDownloadOutline, IoCopy } from 'react-icons/io5';

// ── Attachment helpers ─────────────────────────────────────────────────────────
const ATTACH_PREFIX = '__ATTACH__:';

interface AttachMeta { url: string; filename: string; size: number; mime_type: string; }

function parseAttach(content: string): AttachMeta | null {
    if (!content.startsWith(ATTACH_PREFIX)) return null;
    try { return JSON.parse(content.slice(ATTACH_PREFIX.length)); } catch { return null; }
}

function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const API_BASE = (import.meta.env.VITE_API_URL as string) ?? '';

// ── Emoji Picker data ──────────────────────────────────────────────────────────
const EMOJI_GROUPS: { label: string; emojis: string[] }[] = [
    { label: 'Cảm xúc', emojis: ['😀','😄','😁','😆','😅','🤣','😂','🙂','😊','😇','🥰','😍','🤩','😘','😗','😏','😒','😞','😔','😟','😕','🙁','☹️','😣','😖','😫','😩','🥺','😢','😭','😤','😠','😡','🤬','🤯','😳','🥵','🥶','😱','😨','😰','😥','😓','🤗','🤭','🤫','🤔','🤐','🤨','😐','😑','😶','😏','😬','🙄','😯','😦','😧','😮','😲','🥱','😴','🤤','😪','😵','🤪','😵‍💫','🤑','🤠'] },
    { label: 'Cử chỉ', emojis: ['👋','🤚','🖐️','✋','🖖','👌','🤌','🤏','✌️','🤞','🤟','🤘','🤙','👈','👉','👆','🖕','👇','☝️','👍','👎','✊','👊','🤛','🤜','👏','🙌','👐','🤲','🤝','🙏','✍️','💪','🦾','🖐','🫶','🫱','🫲','🫳','🫴','🫵'] },
    { label: 'Trái tim', emojis: ['❤️','🧡','💛','💚','💙','💜','🖤','🤍','🤎','💔','❣️','💕','💞','💓','💗','💖','💘','💝','💟','☮️','✝️','♾️','💯','🔥','⭐','🌟','✨','💫','🎉','🎊','🎈','🎁','🏆','🥇','🎯','💡','❓','‼️','⁉️','🆗','🆕','🆙','✅','❎','🚫'] },
    { label: 'Vật thể', emojis: ['📱','💻','🖥️','⌨️','🖱️','🖨️','📷','📸','📹','🎥','📞','☎️','📺','📻','🎙️','🎚️','🎛️','⏰','⌚','📡','🔋','💡','🔦','🕯️','🪔','💰','💵','💴','💶','💷','💸','💳','🏧','📝','📄','📋','📊','📈','📉','🗂️','📁','📂','🗃️','🗄️','🗑️','🔑','🔐','🔒','🔓','🔨','⚒️','🛠️','🔧','🔩','⚙️','🗜️','⚖️','🔗','📎','🖇️','📐','📏','✂️','🗡️','🔪','💊','💉','🩺','🔬','🔭'] },
    { label: 'Ký hiệu', emojis: ['👋','✅','❌','⚠️','🔴','🟠','🟡','🟢','🔵','🟣','⚫','⚪','🔺','🔻','🔷','🔶','🔹','🔸','▶️','⏭️','⏩','⏪','⏮️','⏫','⏬','⏏️','🔄','🔃','🔀','🔁','🔂','▶️','⏸️','⏹️','⏺️','🎦','🔊','🔔','🔕','💬','💭','🗨️','📢','📣','🔕','🚨','🚦','🚥'] },
];

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
    replyToContent?: string;
    replyToSender?: string;
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
    const { user, sessionId } = useAuth();
    const { setUnreadMessages, setPendingRequests } = useNotifications();
    // State
    const [contacts, setContacts] = useState<ContactWithStatus[]>([]);
    const [selectedContact, setSelectedContact] = useState<ContactWithStatus | null>(null);
    const [messages, setMessages] = useState<Map<string, ChatMessage[]>>(new Map());
    const [inputMessage, setInputMessage] = useState('');
    const [searchQuery, setSearchQuery] = useState('');
    const [showAddContact, setShowAddContact] = useState(false);
    const [newContactId, setNewContactId] = useState('');
    const [showContactMenu, setShowContactMenu] = useState(false);
    const [dealPanelExpanded, setDealPanelExpanded] = useState(true);
    // Map from contact node_id → deal info, persists across tab navigation
    const [activeDeals, setActiveDeals] = useState<Map<string, DealInfo>>(new Map());
    const [replyingTo, setReplyingTo] = useState<ChatMessage | null>(null);
    const [hoveredMsgId, setHoveredMsgId] = useState<string | null>(null);
    const [showEmojiPicker, setShowEmojiPicker] = useState(false);
    const [emojiPickerPos, setEmojiPickerPos] = useState<{ bottom: number; left: number } | null>(null);
    const [emojiTab, setEmojiTab] = useState(0);
    const [isUploading, setIsUploading] = useState(false);
    const [addContactError, setAddContactError] = useState('');
    const [addContactLoading, setAddContactLoading] = useState(false);
    const [copiedMyId, setCopiedMyId] = useState(false);
    const [pendingFriendRequests, setPendingFriendRequests] = useState<FriendRequestType[]>([]);
    const [showRequestsPanel, setShowRequestsPanel] = useState(false);
    const [requestActionLoading, setRequestActionLoading] = useState<number | null>(null);

    // Refs
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);
    const menuBtnRef = useRef<HTMLButtonElement>(null);
    const emojiPickerRef = useRef<HTMLDivElement>(null);
    const emojiBtnRef = useRef<HTMLButtonElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const contactsRef = useRef<ContactWithStatus[]>([]);
    const selectedContactRef = useRef<ContactWithStatus | null>(null);
    const [menuPos, setMenuPos] = useState<{ top: number; right: number } | null>(null);

    // Keep refs updated
    useEffect(() => {
        contactsRef.current = contacts;
    }, [contacts]);

    useEffect(() => {
        selectedContactRef.current = selectedContact;
    }, [selectedContact]);

    // Close menu / emoji picker when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
                setShowContactMenu(false);
            }
            if (emojiPickerRef.current && !emojiPickerRef.current.contains(event.target as Node)) {
                setShowEmojiPicker(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    // Insert emoji vào textarea
    const handleEmojiClick = (emoji: string) => {
        const el = inputRef.current as unknown as HTMLTextAreaElement | null;
        if (!el) { setInputMessage(prev => prev + emoji); return; }
        const start = el.selectionStart ?? inputMessage.length;
        const end = el.selectionEnd ?? inputMessage.length;
        const next = inputMessage.slice(0, start) + emoji + inputMessage.slice(end);
        setInputMessage(next);
        setTimeout(() => {
            el.selectionStart = el.selectionEnd = start + emoji.length;
            el.focus();
        }, 0);
    };

    // Upload file
    const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!fileInputRef.current) return;
        fileInputRef.current.value = '';
        if (!file || !selectedContact) return;
        if (file.size > 50 * 1024 * 1024) {
            alert('File vượt quá giới hạn 50MB');
            return;
        }
        setIsUploading(true);
        try {
            const uploaded = await uploadChatFile(file);
            const meta = { url: uploaded.url, filename: uploaded.filename, size: uploaded.size, mime_type: uploaded.mime_type };
            const content = ATTACH_PREFIX + JSON.stringify(meta);
            const timestamp = Date.now();
            const msgId = `${timestamp}-${sessionId}-file`;
            const msg: ChatMessage = { id: msgId, senderId: sessionId, senderName: user?.username ?? 'Me', content, timestamp, isMe: true };
            setMessages(prev => {
                const newMap = new Map(prev);
                const existing = newMap.get(selectedContact.node_id) || [];
                if (existing.some(m => m.id === msgId)) return prev;
                newMap.set(selectedContact.node_id, [...existing, msg]);
                return newMap;
            });
            await apiSendChatMessage(selectedContact.node_id, content, sessionId, user?.username ?? 'Guest', sessionId);
        } catch (err) {
            console.error('Upload failed:', err);
            alert('Upload thất bại. Vui lòng thử lại.');
        } finally {
            setIsUploading(false);
        }
    };

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
            } catch (err) {
                console.error('Failed to load contacts:', err);
            }
        };
        loadContacts();
    }, []);

    // Load pending friend requests on mount
    useEffect(() => {
        const load = async () => {
            try {
                const list = await getPendingRequests();
                setPendingFriendRequests(list);
            } catch { /* guest hoặc chưa login — bỏ qua */ }
        };
        load();
    }, []);

    // WS: nhận lời mời kết bạn real-time
    useEffect(() => {
        const unlistenReq = k2ws.listen('k2://friend-request', (event) => {
            const payload = event as any;
            setPendingFriendRequests(prev => {
                if (prev.some(r => r.id === payload.id)) return prev;
                return [{ id: payload.id, from_node_id: payload.from_node_id, from_username: payload.from_username, status: 'pending', created_at: payload.created_at }, ...prev];
            });
            setShowRequestsPanel(true);
        });
        const unlistenRes = k2ws.listen('k2://friend-request-response', (event) => {
            const payload = event as any;
            if (payload.status === 'accepted') {
                // Thêm người đó vào contacts list
                const newContact = { node_id: payload.by_node_id, nickname: payload.by_username, added_at: Date.now(), notes: 'Added via friend request', isOnline: false, unreadCount: 0 };
                setContacts(prev => prev.some(c => c.node_id === newContact.node_id) ? prev : [...prev, newContact]);
            }
        });
        return () => { unlistenReq(); unlistenRes(); };
    }, []);

    // Auto-open chat with a specific contact when navigated from NegotiationDashboard
    useEffect(() => {
        if (!openChatWith) return;

        const autoOpen = async () => {
            // Lưu deal info theo contact node_id để persist qua navigation
            if (openChatWith.deal) setActiveDeals(prev => new Map(prev).set(openChatWith.nodeId, openChatWith.deal!));

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

                // Skip messages from self — dùng sender_session_id (UUID) để so sánh
                if (payload.sender_session_id === sessionId) {
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
                    replyToContent: payload.reply_to_content,
                    replyToSender: payload.reply_to_sender,
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
    }, [sessionId]); // chỉ re-register khi sessionId đổi, không re-register khi chọn contact

    // Scroll when messages change
    useEffect(() => {
        scrollToBottom();
    }, [messages, selectedContact, scrollToBottom]);

    // Sync tổng unread messages lên NotificationContext → Sidebar badge
    useEffect(() => {
        const total = contacts.reduce((sum, c) => sum + (c.unreadCount || 0), 0);
        setUnreadMessages(total);
    }, [contacts, setUnreadMessages]);

    // Sync số pending friend requests lên NotificationContext → Sidebar badge
    useEffect(() => {
        setPendingRequests(pendingFriendRequests.length);
    }, [pendingFriendRequests, setPendingRequests]);

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

        // Check every 10 seconds
        const interval = setInterval(checkOnlineStatus, 10000);
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
        // Ping ngay để cập nhật online status
        try {
            const isOnline = await pingContact(contact.node_id);
            setContacts(prev => prev.map(c =>
                c.node_id === contact.node_id ? { ...c, isOnline } : c
            ));
        } catch { /* ignore */ }

        // Load chat history from backend
        if (sessionId) {
            try {
                const nodes = [sessionId, contact.node_id].sort();
                const conversationId = `p2p_${nodes.join('_')}`;
                const history = await getChatHistory(sessionId, conversationId, 100);
                if (history.messages.length > 0) {
                    const myName = user?.username ?? 'Guest';
                    const historyMessages: ChatMessage[] = history.messages.map(m => ({
                        id: `hist-${m.id}`,
                        senderId: m.sender_name === myName ? sessionId : contact.node_id,
                        senderName: m.sender_name ?? 'Unknown',
                        content: m.content,
                        timestamp: m.created_at,
                        isMe: m.sender_name === myName,
                        replyToContent: m.reply_to_content,
                        replyToSender: m.reply_to_sender,
                    }));
                    setMessages(prev => {
                        const newMap = new Map(prev);
                        // Merge: keep real-time messages that arrived after history
                        const existing = newMap.get(contact.node_id) || [];
                        const existingIds = new Set(existing.map(m => m.content + m.timestamp));
                        const merged = [
                            ...historyMessages,
                            ...existing.filter(m => !existingIds.has(m.content + m.timestamp) || m.id.startsWith('hist-')),
                        ];
                        // Deduplicate by timestamp+content, keep order
                        const seen = new Set<string>();
                        const deduped = merged.filter(m => {
                            const key = `${m.timestamp}-${m.content}-${m.isMe}`;
                            if (seen.has(key)) return false;
                            seen.add(key);
                            return true;
                        });
                        newMap.set(contact.node_id, deduped);
                        return newMap;
                    });
                }
            } catch (err) {
                console.error('[NegotiationChat] Failed to load chat history:', err);
            }
        }

        inputRef.current?.focus();
    };

    // Send message
    const handleSendMessage = async () => {
        if (!inputMessage.trim() || !selectedContact) return;

        const timestamp = Date.now();
        const msgId = `${timestamp}-${sessionId}`;
        const content = inputMessage.trim();
        const currentReply = replyingTo;

        const msg: ChatMessage = {
            id: msgId,
            senderId: sessionId,
            senderName: 'Me',
            content,
            timestamp,
            isMe: true,
            replyToContent: currentReply?.content,
            replyToSender: currentReply?.isMe ? 'Bạn' : currentReply?.senderName,
        };

        setMessages(prev => {
            const newMap = new Map(prev);
            const existing = newMap.get(selectedContact.node_id) || [];
            if (existing.some(m => m.id === msgId)) return prev;
            newMap.set(selectedContact.node_id, [...existing, msg]);
            return newMap;
        });

        setInputMessage('');
        setReplyingTo(null);

        try {
            await apiSendChatMessage(
                selectedContact.node_id,
                content,
                sessionId,
                user?.username ?? 'Guest',
                sessionId,
                currentReply?.content,
                currentReply?.isMe ? 'Bạn' : currentReply?.senderName,
            );
        } catch (err) {
            console.error('Send failed:', err);
        }
    };

    // Gửi lời mời kết bạn
    const handleAddContact = async () => {
        if (!newContactId.trim()) return;

        const nodeIdTrimmed = newContactId.trim();
        if (nodeIdTrimmed === sessionId) {
            setAddContactError('Không thể gửi lời mời cho chính mình.');
            return;
        }
        if (contacts.some(c => c.node_id === nodeIdTrimmed)) {
            setAddContactError('Người này đã là bạn bè của bạn.');
            return;
        }

        setAddContactLoading(true);
        setAddContactError('');
        try {
            await sendFriendRequest(nodeIdTrimmed);
            setShowAddContact(false);
            setNewContactId('');
            setAddContactError('');
            // Hiện thông báo nhỏ (dùng alert tạm, có thể thay toast sau)
            alert('Đã gửi lời mời kết bạn! Chờ người kia xác nhận.');
        } catch (err: any) {
            const msg = err?.message || String(err);
            if (msg.includes('401') || msg.includes('Unauthorized')) {
                setAddContactError('Bạn cần đăng nhập để gửi lời mời.');
            } else if (msg.includes('409') || msg.includes('already')) {
                setAddContactError('Đã gửi lời mời cho người này rồi.');
            } else {
                setAddContactError(`Gửi thất bại: ${msg}`);
            }
        } finally {
            setAddContactLoading(false);
        }
    };

    // Chấp nhận lời mời kết bạn
    const handleAcceptRequest = async (req: FriendRequestType) => {
        setRequestActionLoading(req.id);
        try {
            const result = await apiAcceptRequest(req.id);
            setPendingFriendRequests(prev => prev.filter(r => r.id !== req.id));
            const c = result.contact;
            setContacts(prev => prev.some(x => x.node_id === c.node_id) ? prev : [
                ...prev, { node_id: c.node_id, nickname: c.nickname, added_at: c.added_at, notes: 'Added via friend request', isOnline: false, unreadCount: 0 }
            ]);
        } catch (err) {
            console.error('Accept failed:', err);
        } finally {
            setRequestActionLoading(null);
        }
    };

    // Từ chối lời mời kết bạn
    const handleDeclineRequest = async (req: FriendRequestType) => {
        setRequestActionLoading(req.id);
        try {
            await apiDeclineRequest(req.id);
            setPendingFriendRequests(prev => prev.filter(r => r.id !== req.id));
        } catch (err) {
            console.error('Decline failed:', err);
        } finally {
            setRequestActionLoading(null);
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
    const handleClearChat = async () => {
        if (!selectedContact || !sessionId) return;

        const confirmClear = window.confirm(`Xóa toàn bộ tin nhắn với "${selectedContact.nickname}"?`);
        if (!confirmClear) return;

        const nodes = [sessionId, selectedContact.node_id].sort();
        const conversationId = `p2p_${nodes.join('_')}`;

        try {
            await clearChatHistory(sessionId, conversationId);
        } catch (err) {
            console.error('[NegotiationChat] Failed to clear chat history on server:', err);
        }

        setMessages(prev => {
            const newMap = new Map(prev);
            newMap.delete(selectedContact.node_id);
            return newMap;
        });
        setShowContactMenu(false);
    };

    // Get messages for selected contact
    const currentMessages = selectedContact ? (messages.get(selectedContact.node_id) || []) : [];
    // Get deal for selected contact
    const activeDeal = selectedContact ? (activeDeals.get(selectedContact.node_id) ?? null) : null;

    return (
        <div className="negotiation-chat">
            {/* Left Sidebar - Contact List (Discord-style) */}
            <div className="chat-sidebar">
                <div className="sidebar-header">
                    <h3>Direct Messages</h3>
                    <div style={{ display: 'flex', gap: 6 }}>
                        {pendingFriendRequests.length > 0 && (
                            <button
                                className={`add-contact-btn ${showRequestsPanel ? 'active' : ''}`}
                                onClick={() => setShowRequestsPanel(o => !o)}
                                title="Lời mời kết bạn"
                                style={{ position: 'relative' }}
                            >
                                <IoPersonAdd />
                                <span className="fr-badge">{pendingFriendRequests.length}</span>
                            </button>
                        )}
                        <button
                            className="add-contact-btn"
                            onClick={() => setShowAddContact(true)}
                            title="Gửi lời mời kết bạn"
                        >
                            <IoPersonAdd />
                        </button>
                    </div>
                </div>

                {/* Pending Friend Requests Panel */}
                {showRequestsPanel && pendingFriendRequests.length > 0 && (
                    <div className="fr-panel">
                        <div className="fr-panel-title">Lời mời kết bạn ({pendingFriendRequests.length})</div>
                        {pendingFriendRequests.map(req => (
                            <div key={req.id} className="fr-item">
                                <div className="fr-avatar" style={{ background: getAvatarColor(req.from_username) }}>
                                    {getInitials(req.from_username)}
                                </div>
                                <div className="fr-info">
                                    <span className="fr-name">{req.from_username}</span>
                                    <span className="fr-node">{req.from_node_id.slice(0, 10)}…</span>
                                </div>
                                <div className="fr-actions">
                                    <button
                                        className="fr-accept-btn"
                                        title="Chấp nhận"
                                        disabled={requestActionLoading === req.id}
                                        onClick={() => handleAcceptRequest(req)}
                                    >
                                        <IoCheckmark />
                                    </button>
                                    <button
                                        className="fr-decline-btn"
                                        title="Từ chối"
                                        disabled={requestActionLoading === req.id}
                                        onClick={() => handleDeclineRequest(req)}
                                    >
                                        <IoClose />
                                    </button>
                                </div>
                            </div>
                        ))}
                    </div>
                )}

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
                            <button
                                ref={menuBtnRef}
                                className="menu-btn"
                                onClick={() => {
                                    if (!showContactMenu && menuBtnRef.current) {
                                        const rect = menuBtnRef.current.getBoundingClientRect();
                                        setMenuPos({ top: rect.bottom + 6, right: window.innerWidth - rect.right });
                                    }
                                    setShowContactMenu(v => !v);
                                }}
                            >
                                <IoEllipsisVertical />
                            </button>
                            {/* Contact Menu Dropdown — position:fixed to escape any overflow/stacking context */}
                            {showContactMenu && menuPos && (
                                <div
                                    className="contact-menu-dropdown"
                                    ref={menuRef}
                                    style={{ position: 'fixed', top: menuPos.top, right: menuPos.right }}
                                >
                                    <button className="menu-item" onClick={handleClearChat}>
                                        <IoTrashOutline /> Xóa tin nhắn
                                    </button>
                                    <button className="menu-item danger" onClick={handleDeleteContact}>
                                        <IoPersonRemoveOutline /> Xóa liên hệ
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
                                        onMouseEnter={() => setHoveredMsgId(msg.id)}
                                        onMouseLeave={() => setHoveredMsgId(null)}
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
                                            {msg.replyToContent && (
                                                <div className="reply-preview">
                                                    <span className="reply-preview-sender">{msg.replyToSender}</span>
                                                    <span className="reply-preview-text">{msg.replyToContent}</span>
                                                </div>
                                            )}
                                            {(() => {
                                                const attach = parseAttach(msg.content);
                                                if (!attach) return <span className="message-content">{msg.content}</span>;
                                                const isImage = attach.mime_type.startsWith('image/');
                                                const src = attach.url.startsWith('http') ? attach.url : `${API_BASE}${attach.url}`;
                                                return isImage ? (
                                                    <div className="msg-image-wrap">
                                                        <img src={src} alt={attach.filename} className="msg-image" onClick={() => window.open(src, '_blank')} />
                                                        <span className="msg-image-name">{attach.filename}</span>
                                                    </div>
                                                ) : (
                                                    <a className="msg-file-attach" href={src} download={attach.filename} target="_blank" rel="noreferrer">
                                                        <IoDocumentOutline className="msg-file-icon" />
                                                        <div className="msg-file-info">
                                                            <span className="msg-file-name">{attach.filename}</span>
                                                            <span className="msg-file-size">{formatBytes(attach.size)}</span>
                                                        </div>
                                                        <IoDownloadOutline className="msg-file-dl" />
                                                    </a>
                                                );
                                            })()}
                                            <span className="message-time">{formatTime(msg.timestamp)}</span>
                                        </div>
                                        {hoveredMsgId === msg.id && (
                                            <button
                                                className="reply-btn"
                                                onClick={() => setReplyingTo(msg)}
                                                title="Trả lời"
                                            >
                                                <IoReturnUpBack />
                                            </button>
                                        )}
                                    </div>
                                ))
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        {/* Reply Bar */}
                        {replyingTo && (
                            <div className="reply-bar">
                                <div className="reply-bar-content">
                                    <span className="reply-bar-sender">
                                        {replyingTo.isMe ? 'Bạn' : replyingTo.senderName}
                                    </span>
                                    <span className="reply-bar-text">{replyingTo.content}</span>
                                </div>
                                <button className="reply-cancel-btn" onClick={() => setReplyingTo(null)}>
                                    <IoClose />
                                </button>
                            </div>
                        )}

                        {/* Input Area */}
                        <div className="chat-input-area">
                            {/* Hidden file input */}
                            <input
                                ref={fileInputRef}
                                type="file"
                                accept="image/*,video/*,application/pdf,.zip,.txt,.json,.doc,.docx,.xls,.xlsx,.ppt,.pptx"
                                style={{ display: 'none' }}
                                onChange={handleFileChange}
                            />

                            <div className="input-actions-left" style={{ position: 'relative' }}>
                                {/* File attach button */}
                                <button
                                    className={`input-icon-btn ${isUploading ? 'uploading' : ''}`}
                                    title="Đính kèm file/ảnh (tối đa 50MB)"
                                    onClick={() => fileInputRef.current?.click()}
                                    disabled={isUploading}
                                >
                                    {isUploading ? <span className="upload-spinner" /> : <IoAttachOutline />}
                                </button>

                                {/* Emoji button */}
                                <button
                                    ref={emojiBtnRef}
                                    className={`input-icon-btn ${showEmojiPicker ? 'active' : ''}`}
                                    title="Emoji"
                                    onClick={() => {
                                        if (!showEmojiPicker && emojiBtnRef.current) {
                                            const r = emojiBtnRef.current.getBoundingClientRect();
                                            setEmojiPickerPos({
                                                bottom: window.innerHeight - r.top + 10,
                                                left: r.left,
                                            });
                                        }
                                        setShowEmojiPicker(o => !o);
                                    }}
                                >
                                    <IoHappyOutline />
                                </button>
                            </div>

                            {/* Emoji picker — rendered via portal to escape backdrop-filter stacking context */}
                            {showEmojiPicker && emojiPickerPos && ReactDOM.createPortal(
                                <div
                                    className="emoji-picker"
                                    ref={emojiPickerRef}
                                    style={{ position: 'fixed', bottom: emojiPickerPos.bottom, left: emojiPickerPos.left }}
                                >
                                    <div className="emoji-picker-header">
                                        <span className="emoji-picker-title">✨ Emoji</span>
                                    </div>
                                    <div className="emoji-tabs">
                                        {EMOJI_GROUPS.map((g, i) => (
                                            <button
                                                key={i}
                                                className={`emoji-tab ${emojiTab === i ? 'active' : ''}`}
                                                onClick={() => setEmojiTab(i)}
                                                title={g.label}
                                            >
                                                {g.emojis[0]}
                                                <span className="emoji-tab-label">{g.label}</span>
                                            </button>
                                        ))}
                                    </div>
                                    <div className="emoji-grid">
                                        {EMOJI_GROUPS[emojiTab].emojis.map((emoji, i) => (
                                            <button
                                                key={i}
                                                className="emoji-btn"
                                                onClick={() => { handleEmojiClick(emoji); setShowEmojiPicker(false); }}
                                            >
                                                {emoji}
                                            </button>
                                        ))}
                                    </div>
                                </div>,
                                document.body
                            )}

                            <textarea
                                ref={inputRef as any}
                                className="chat-input-textarea"
                                placeholder={`Nhắn tin cho ${selectedContact.nickname}...`}
                                value={inputMessage}
                                rows={1}
                                onChange={(e) => setInputMessage(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === 'Enter' && !e.shiftKey) {
                                        e.preventDefault();
                                        handleSendMessage();
                                    }
                                }}
                            />
                            <button
                                className={`send-btn ${inputMessage.trim() ? 'active' : ''}`}
                                onClick={handleSendMessage}
                                disabled={!inputMessage.trim()}
                                title="Gửi (Enter)"
                            >
                                <IoSend />
                            </button>
                        </div>
                    </>
                ) : (
                    <div className="no-chat-selected">
                        <div className="empty-state">
                            <div className="empty-state-icon">
                                <svg width="40" height="40" viewBox="0 0 24 24" fill="currentColor">
                                    <path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H6l-2 2V4h16v12z" />
                                </svg>
                            </div>
                            <h3>Chọn một cuộc trò chuyện</h3>
                            <p>Chọn liên hệ từ danh sách bên trái để bắt đầu đàm phán P2P</p>
                        </div>
                    </div>
                )}
            </div>

            {/* Add Contact Modal */}
            {showAddContact && (
                <div className="modal-overlay" onClick={() => { setShowAddContact(false); setAddContactError(''); }}>
                    <div className="add-contact-modal" onClick={e => e.stopPropagation()}>
                        <div className="modal-header">
                            <h3>Thêm liên hệ mới</h3>
                            <button className="close-btn" onClick={() => { setShowAddContact(false); setAddContactError(''); }}>
                                <IoClose />
                            </button>
                        </div>
                        <div className="modal-body">
                            {/* My own node ID — for sharing */}
                            <div className="my-node-id-box">
                                <span className="my-node-id-label">Node ID của bạn</span>
                                <div className="my-node-id-row">
                                    <code className="my-node-id-value">{sessionId ? `${sessionId.slice(0, 16)}...${sessionId.slice(-8)}` : '—'}</code>
                                    <button
                                        className="my-node-id-copy"
                                        title={copiedMyId ? 'Đã sao chép!' : 'Sao chép'}
                                        onClick={() => {
                                            if (!sessionId) return;
                                            navigator.clipboard.writeText(sessionId);
                                            setCopiedMyId(true);
                                            setTimeout(() => setCopiedMyId(false), 2000);
                                        }}
                                    >
                                        {copiedMyId ? <IoCheckmark /> : <IoCopy />}
                                    </button>
                                </div>
                                <span className="my-node-id-hint">Chia sẻ ID này để người kia gửi lời mời cho bạn</span>
                            </div>

                            <div className="form-group">
                                <label>Node ID của đối tác</label>
                                <input
                                    type="text"
                                    placeholder="Dán Node ID của đối tác vào đây..."
                                    value={newContactId}
                                    onChange={(e) => { setNewContactId(e.target.value); setAddContactError(''); }}
                                    onKeyDown={(e) => { if (e.key === 'Enter') handleAddContact(); }}
                                    autoFocus
                                />
                            </div>
                            {addContactError && (
                                <div className="add-contact-error">{addContactError}</div>
                            )}
                        </div>
                        <div className="modal-footer">
                            <button className="cancel-btn" onClick={() => { setShowAddContact(false); setAddContactError(''); }}>
                                Hủy
                            </button>
                            <button
                                className="confirm-btn"
                                onClick={handleAddContact}
                                disabled={!newContactId.trim() || addContactLoading}
                            >
                                {addContactLoading ? '...' : <><IoPersonAdd /> Gửi lời mời</>}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};

export default NegotiationChat;
