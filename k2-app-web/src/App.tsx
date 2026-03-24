import { useState, useEffect } from "react";
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  useNavigate,
  useLocation,
} from "react-router-dom";
import { initNode as apiInitNode, k2ws } from "./api";
import { Sidebar, Header } from "./components";
import { MarketplacePage, NegotiationPage, ContactPage, ProfilePage } from "./pages";
import { AuthGate } from "./pages/Auth/AuthGate";
import ChatInterface from "./components/Chat/ChatInterface";
import { AuthProvider, useAuth } from "./context/AuthContext";
import { NotificationProvider, useNotifications } from "./context/NotificationContext";
import { TabType, TAB_LABELS } from "./types";
import "./App.css";

function AppShell() {
  const { mode, user, sessionId, loginAsGuest, loginWithToken, logout } = useAuth();
  const { unreadMessages, pendingRequests, clearUnread } = useNotifications();
  const navigate = useNavigate();
  const location = useLocation();

  const [isChatOpen, setIsChatOpen] = useState(false);
  const [chatWidth, setChatWidth] = useState(() => {
    const saved = localStorage.getItem('k2-chat-width');
    return saved ? parseInt(saved) : 380;
  });
  const [openChatWith, setOpenChatWith] = useState<{ nodeId: string; name: string; deal?: { title: string; priceMin: number; priceMax: number; currency: string } } | null>(null);

  // Listen for k2:openChat event from NegotiationDashboard
  useEffect(() => {
    const handler = (e: CustomEvent<typeof openChatWith>) => {
      setOpenChatWith(e.detail);
      navigate('/negotiation');
    };
    window.addEventListener('k2:openChat' as any, handler);
    return () => window.removeEventListener('k2:openChat' as any, handler);
  }, [navigate]);

  // Navigate về marketplace khi AI tạo form (MarketplacePage luôn mounted nên listener vẫn active)
  useEffect(() => {
    const handler = () => navigate('/marketplace');
    window.addEventListener('k2:showDynamicForm' as any, handler);
    return () => window.removeEventListener('k2:showDynamicForm' as any, handler);
  }, [navigate]);

  // Init node khi đã vào app (guest hoặc auth)
  useEffect(() => {
    if (mode !== "guest" && mode !== "auth") return;
    const init = async () => {
      try {
        await apiInitNode();
        k2ws.connect(undefined, sessionId, sessionId);
      } catch (err) {
        console.error("Init node failed:", err);
      }
    };
    init();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  // Derive activeTab from current URL path
  const pathSegment = location.pathname.replace("/", "") || "marketplace";
  const activeTab = (pathSegment in TAB_LABELS ? pathSegment : "marketplace") as TabType;

  if (mode === "loading") {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', background: '#1a1a2e' }}>
        <div style={{ width: 36, height: 36, border: '3px solid #4DA6FF', borderTopColor: 'transparent', borderRadius: '50%', animation: 'spin 0.8s linear infinite' }} />
        <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
      </div>
    );
  }

  if (mode === "pending") {
    return (
      <AuthGate
        onGuest={loginAsGuest}
        onAuth={loginWithToken}
      />
    );
  }

  return (
    <div className="app-layout">
      <Sidebar
        activeTab={activeTab}
        onTabChange={(tab) => {
          if (tab === 'negotiation') clearUnread();
          navigate(`/${tab}`);
        }}
        badges={{ negotiation: unreadMessages, contact: pendingRequests }}
      />
      <main className="main-content">
        <Header
          title={TAB_LABELS[activeTab]}
          teamName={user?.username ?? "guest"}
          onLogout={mode === "auth" ? logout : undefined}
          onOpenAIChat={activeTab === 'negotiation' ? () => setIsChatOpen(o => !o) : undefined}
          isAIChatOpen={isChatOpen}
        />
        <div className="page-content">
          {/* Luôn mount MarketplacePage để giữ toàn bộ state (form, offers, negotiation) */}
          <div style={{ display: activeTab === 'marketplace' ? 'contents' : 'none' }}>
            <MarketplacePage />
          </div>
          {/* Luôn mount NegotiationPage để giữ state messages/contacts */}
          <div style={{ display: activeTab === 'negotiation' ? 'contents' : 'none' }}>
            <NegotiationPage openChatWith={openChatWith} onChatOpened={() => setOpenChatWith(null)} />
          </div>
          <Routes>
            <Route path="/" element={<Navigate to="/marketplace" replace />} />
            <Route path="/marketplace" element={null} />
            <Route path="/contact" element={<ContactPage />} />
            <Route path="/profile" element={<ProfilePage />} />
            <Route path="/negotiation" element={null} />
            <Route path="*" element={<Navigate to="/marketplace" replace />} />
          </Routes>
        </div>
      </main>
      <ChatInterface
        isOpen={isChatOpen}
        onToggle={() => setIsChatOpen(!isChatOpen)}
        width={chatWidth}
        sessionId={sessionId}
        hideFab={activeTab === 'negotiation'}
        onWidthChange={(w) => {
          setChatWidth(w);
          localStorage.setItem('k2-chat-width', w.toString());
        }}
      />
    </div>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <NotificationProvider>
        <BrowserRouter>
          <AppShell />
        </BrowserRouter>
      </NotificationProvider>
    </AuthProvider>
  );
}
