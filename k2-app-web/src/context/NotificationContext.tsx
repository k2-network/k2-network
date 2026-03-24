import { createContext, useContext, useState, useCallback } from 'react';

interface NotificationContextValue {
    /** Tổng tin nhắn P2P chưa đọc */
    unreadMessages: number;
    /** Số lời mời kết bạn đang chờ */
    pendingRequests: number;
    setUnreadMessages: (n: number) => void;
    setPendingRequests: (n: number) => void;
    clearUnread: () => void;
    clearPendingRequests: () => void;
}

const NotificationContext = createContext<NotificationContextValue | null>(null);

export function NotificationProvider({ children }: { children: React.ReactNode }) {
    const [unreadMessages, setUnreadMessages] = useState(0);
    const [pendingRequests, setPendingRequests] = useState(0);

    const clearUnread = useCallback(() => setUnreadMessages(0), []);
    const clearPendingRequests = useCallback(() => setPendingRequests(0), []);

    return (
        <NotificationContext.Provider value={{
            unreadMessages,
            pendingRequests,
            setUnreadMessages,
            setPendingRequests,
            clearUnread,
            clearPendingRequests,
        }}>
            {children}
        </NotificationContext.Provider>
    );
}

export function useNotifications() {
    const ctx = useContext(NotificationContext);
    if (!ctx) throw new Error('useNotifications must be used inside NotificationProvider');
    return ctx;
}
