import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sidebar, Header } from "./components";
import { MarketplacePage, NegotiationPage, ContactPage, ProfilePage, SyncPage } from "./pages";
import ChatInterface from "./components/Chat/ChatInterface";
import { TabType, TAB_LABELS } from "./types";
import "./App.css";

function App() {
  const [activeTab, setActiveTab] = useState<TabType>("marketplace");
  const [nodeId, setNodeId] = useState<string>("");
  const [_initStatus, setInitStatus] = useState("Initializing...");
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [chatWidth, setChatWidth] = useState(() => {
    const saved = localStorage.getItem('k2-chat-width');
    return saved ? parseInt(saved) : 380;
  });

  useEffect(() => {
    const initNode = async () => {
      try {
        const shortId = await invoke<string>("init_node");
        setNodeId(shortId);
        setInitStatus("Ready");
      } catch (err) {
        setInitStatus(`Failed: ${err}`);
      }
    };
    initNode();
  }, []);

  const renderContent = () => {
    switch (activeTab) {
      case "marketplace":
        return <MarketplacePage />;
      case "negotiation":
        return <NegotiationPage />;
      case "contact":
        return <ContactPage />;
      case "profile":
        return <ProfilePage />;
      case "sync":
        return <SyncPage />;
      default:
        return <MarketplacePage />;
    }
  };

  return (
    <div className="app-layout">
      <Sidebar activeTab={activeTab} onTabChange={setActiveTab} />
      <main className="main-content">
        <Header
          title={TAB_LABELS[activeTab]}
          nodeId={nodeId}
          teamName="k2-team"
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

export default App;
