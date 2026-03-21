import { useState, useEffect } from "react";
import { initNode as apiInitNode, k2ws } from "./api";
import { Sidebar, Header } from "./components";
import { MarketplacePage, NegotiationPage, ContactPage, ProfilePage } from "./pages";
import { AuthGate } from "./pages/Auth/AuthGate";
import ChatInterface from "./components/Chat/ChatInterface";
import { AuthProvider, useAuth } from "./context/AuthContext";
import { TabType, TAB_LABELS } from "./types";
import "./App.css";

function AppShell() {
  const { mode, user, sessionId, loginAsGuest, loginWithToken } = useAuth();
  const [activeTab, setActiveTab] = useState<TabType>("marketplace");
  const [nodeId, setNodeId] = useState<string>("");
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
      setActiveTab('negotiation');
    };
    window.addEventListener('k2:openChat' as any, handler);
    return () => window.removeEventListener('k2:openChat' as any, handler);
  }, []);

  // Init node khi đã vào app (guest hoặc auth)
  useEffect(() => {
    if (mode !== "guest" && mode !== "auth") return;
    // Connect WS với sessionId duy nhất của user/guest
    k2ws.connect(undefined, sessionId);
    const init = async () => {
      try {
        const result = await apiInitNode();
        if (mode === "guest") setNodeId(result.node_id);
      } catch (err) {
        console.error("Init node failed:", err);
      }
    };
    init();
  }, [mode, sessionId]);

  // Khi login, dùng node_id từ account
  useEffect(() => {
    if (user?.nodeId) setNodeId(user.nodeId);
  }, [user]);

  const renderContent = () => (
    <>
      {/* Luôn mount NegotiationPage để giữ state messages/contacts, chỉ ẩn/hiện bằng CSS */}
      <div style={{ display: activeTab === 'negotiation' ? 'contents' : 'none' }}>
        <NegotiationPage openChatWith={openChatWith} onChatOpened={() => setOpenChatWith(null)} />
      </div>
      {activeTab === 'marketplace' && <MarketplacePage />}
      {activeTab === 'contact'     && <ContactPage />}
      {activeTab === 'profile'     && <ProfilePage />}
    </>
  );

  // Hiện AuthGate khi chưa chọn chế độ
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
      <Sidebar activeTab={activeTab} onTabChange={setActiveTab} />
      <main className="main-content">
        <Header
          title={TAB_LABELS[activeTab]}
          nodeId={nodeId}
          teamName={user?.username ?? "guest"}
        />
        <div className="page-content">
          {renderContent()}
        </div>
      </main>
      <ChatInterface
        isOpen={isChatOpen}
        onToggle={() => setIsChatOpen(!isChatOpen)}
        width={chatWidth}
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
      <AppShell />
    </AuthProvider>
  );
}
