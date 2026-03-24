import React, { useState, useRef, useEffect, useCallback } from 'react';
import { IoClose, IoSettingsOutline } from 'react-icons/io5';
import ReactMarkdown from 'react-markdown';
import { useGroqChat } from '../../hooks/useGroqChat';
import './ChatInterface.css';
import aiAgentIconLight from '../../assets/icons/ai-agent-large.svg';
import aiAgentIconDark from '../../assets/icons/ai-agent-large-dark.svg';
import { StartTransactionButton } from './StartTransactionButton';

const DEFAULT_SUGGESTIONS = [
  "Tôi muốn mua video",
  "Bán dịch vụ thiết kế UI/UX",
  "Trao đổi laptop cũ",
  "Giới thiệu về K2 Marketplace"
];

const MarkdownContent: React.FC<{ content: string }> = ({ content }) => {
  return (
    <div className="markdown-content">
      <ReactMarkdown
        components={{
          h1: ({ children }) => <h3 style={{ marginTop: 12, marginBottom: 6, color: '#cccccc' }}>{children}</h3>,
          h2: ({ children }) => <h4 style={{ marginTop: 10, marginBottom: 4, color: '#cccccc' }}>{children}</h4>,
          h3: ({ children }) => <h5 style={{ marginTop: 8, marginBottom: 4, color: '#cccccc' }}>{children}</h5>,
          p: ({ children }) => <p style={{ margin: 0 }}>{children}</p>,
          ul: ({ children }) => <ul style={{ marginLeft: 14, marginBottom: 6 }}>{children}</ul>,
          ol: ({ children }) => <ol style={{ marginLeft: 14, marginBottom: 6 }}>{children}</ol>,
          li: ({ children }) => <li style={{ marginBottom: 3 }}>{children}</li>,
          code: (props: any) => {
            const { children, className } = props;
            const isInline = !className;
            return isInline ? (
              <code style={{ background: '#3c3c3c', padding: '2px 6px', borderRadius: 3, fontSize: '0.9em', color: '#ce9178' }}>
                {children}
              </code>
            ) : (
              <pre style={{ background: '#1a1a1a', padding: 10, borderRadius: 6, overflow: 'auto', fontSize: '0.85em', border: '1px solid #3c3c3c' }}>
                <code style={{ color: '#d4d4d4' }}>{children}</code>
              </pre>
            );
          },
          table: ({ children }) => <table style={{ borderCollapse: 'collapse', width: '100%', marginBottom: 10, fontSize: '0.85em' }}>{children}</table>,
          th: ({ children }) => <th style={{ border: '1px solid #3c3c3c', padding: '6px 10px', background: '#2d2d30', textAlign: 'left', color: '#cccccc' }}>{children}</th>,
          td: ({ children }) => <td style={{ border: '1px solid #3c3c3c', padding: '6px 10px', color: '#cccccc' }}>{children}</td>,
          strong: ({ children }) => <strong style={{ fontWeight: 600, color: '#e0e0e0' }}>{children}</strong>,
          blockquote: ({ children }) => <blockquote style={{ borderLeft: '3px solid #0078d4', paddingLeft: 12, margin: '8px 0', color: '#9d9d9d' }}>{children}</blockquote>,
          hr: () => <hr style={{ margin: '12px 0', borderColor: '#3c3c3c', borderWidth: 1 }} />,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
};

interface ChatInterfaceProps {
  isOpen: boolean;
  onToggle: () => void;
  width: number;
  onWidthChange: (width: number) => void;
}

const ChatInterface: React.FC<ChatInterfaceProps> = ({ isOpen, onToggle, width, onWidthChange }) => {
  const [showSettings, setShowSettings] = useState(false);
  const [theme] = useState<'dark' | 'light'>('dark');
  const [isResizing, setIsResizing] = useState(false);
  const [inputValue, setInputValue] = useState('');
  const [injectedMessages, setInjectedMessages] = useState<Array<{
    id: string;
    type: 'startButton' | 'negotiationResult';
    content: any;
    createdAt: string;
  }>>([]);

  const { messages, sendMessage, isProcessing, apiKey, setApiKey } = useGroqChat();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  // Parse form data from AI response
  useEffect(() => {
    if (messages.length === 0) return;
    const lastMessage = messages[messages.length - 1];
    if (lastMessage.role !== 'assistant') return;

    const text = lastMessage.content;
    const formDataMatch = text.match(/<!-- FORM_DATA_START -->\s*([\s\S]*?)\s*<!-- FORM_DATA_END -->/);
    if (formDataMatch) {
      try {
        const formData = JSON.parse(formDataMatch[1]);
        window.dispatchEvent(new CustomEvent('k2:showDynamicForm', {
          detail: { data: formData, streaming: false }
        }));
      } catch (e) {
        console.error('Failed to parse form data:', e);
      }
    }
  }, [messages]);

  // Listen for start button event
  useEffect(() => {
    const handler = (event: CustomEvent<{ actionText: string; title: string }>) => {
      setInjectedMessages(prev => [...prev, {
        id: `start-btn-${Date.now()}`,
        type: 'startButton',
        content: event.detail,
        createdAt: new Date().toISOString()
      }]);
    };
    window.addEventListener('k2:showStartButton' as any, handler);
    return () => window.removeEventListener('k2:showStartButton' as any, handler);
  }, []);

  // Listen for negotiation complete
  const lastNegotiationRef = useRef<string | null>(null);
  useEffect(() => {
    const handler = (event: CustomEvent<{ candidates: any[]; formData: any }>) => {
      const { candidates, formData } = event.detail;
      const eventKey = `${formData?.title || ''}_${candidates.length}_${Date.now().toString().slice(0, -3)}`;
      if (lastNegotiationRef.current === eventKey.slice(0, -1)) return;
      lastNegotiationRef.current = eventKey.slice(0, -1);

      const topCandidates = candidates.slice(0, 3);
      let summaryText = `**Kết quả đàm phán**\n\n`;
      summaryText += `Tôi đã hoàn thành phân tích **${candidates.length}** ứng viên cho yêu cầu "${formData?.title || 'của bạn'}".\n\n`;
      topCandidates.forEach((c: any, i: number) => {
        const score = Math.round(c.negotiationScore || (c.matchScore * 100));
        const priceText = c.priceRange ? `$${c.priceRange.min.toLocaleString()} - $${c.priceRange.max.toLocaleString()}` : 'N/A';
        summaryText += `**#${i + 1} ${c.name} - ${score}%**\n\n`;
        if (c.title) summaryText += `- **Tiêu đề:** ${c.title}\n`;
        summaryText += `- **Giá:** ${priceText}\n`;
        if (c.aiNotes) summaryText += `- **Ghi chú:** ${c.aiNotes}\n`;
        summaryText += `\n`;
      });
      const best = topCandidates[0];
      if (best) {
        const bestScore = Math.round(best.negotiationScore || (best.matchScore * 100));
        summaryText += `**Đề xuất**\n\n`;
        if (bestScore >= 80) summaryText += `**${best.name}** là lựa chọn tốt nhất! Điểm phù hợp cao (${bestScore}%).\n\n`;
        else if (bestScore >= 60) summaryText += `**${best.name}** khá phù hợp. Có thể thương lượng thêm.\n\n`;
        else summaryText += `Các ứng viên cần được cân nhắc kỹ. Bạn có thể mở rộng tiêu chí.\n\n`;
      }

      setInjectedMessages(prev => [...prev, {
        id: `neg-result-${Date.now()}`,
        type: 'negotiationResult',
        content: summaryText,
        createdAt: new Date().toISOString()
      }]);
      setTimeout(() => messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' }), 100);
    };
    window.addEventListener('k2:negotiationComplete' as any, handler);
    return () => window.removeEventListener('k2:negotiationComplete' as any, handler);
  }, []);

  // Resize logic
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
  }, []);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;
      const newWidth = window.innerWidth - e.clientX;
      if (newWidth >= 280 && newWidth <= 600) onWidthChange(newWidth);
    };
    const handleMouseUp = () => setIsResizing(false);
    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    }
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizing, onWidthChange]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, injectedMessages, isProcessing]);

  const handleSubmit = async () => {
    if (!inputValue.trim() || isProcessing) return;
    const msg = inputValue;
    setInputValue('');
    await sendMessage(msg);
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const aiAgentIcon = theme === 'dark' ? aiAgentIconDark : aiAgentIconLight;

  // Merge chat messages with injected messages
  const allItems = [
    ...messages.map(msg => ({
      id: msg.id,
      type: 'chatMessage' as const,
      data: msg,
      createdAt: msg.id
    })),
    ...injectedMessages.map(inj => ({
      id: inj.id,
      type: inj.type,
      data: inj.content,
      createdAt: inj.createdAt
    }))
  ];

  return (
    <>
      {!isOpen && (
        <button
          className="chat-fab-button"
          onClick={onToggle}
          style={{ backgroundImage: `url(${aiAgentIcon})` }}
          aria-label="Open AI Assistant"
        />
      )}

      {isOpen && (
        <div ref={panelRef} className={`chat-panel ${theme}`} style={{ width: `${width}px` }}>
          <div className="chat-resize-handle" onMouseDown={handleMouseDown} />

          <div className="chat-header">
            <div className="chat-header-info">
              <img src={aiAgentIcon} alt="AI Agent" className="ai-agent-large" />
              <div className="chat-title">
                <h3>K2 Assistant</h3>
                <p>{isProcessing ? "Đang trả lời..." : "Sẵn sàng hỗ trợ"}</p>
              </div>
            </div>
            <div style={{ display: 'flex', gap: '4px' }}>
              <button className="chat-settings-btn" onClick={() => setShowSettings(!showSettings)}>
                <IoSettingsOutline size={20} />
              </button>
              <button className="chat-close-btn" onClick={onToggle}>
                <IoClose size={20} />
              </button>
            </div>
          </div>

          {showSettings ? (
            <div className="api-key-overlay">
              <div className="settings-section">
                <label className="settings-label">Groq API Key</label>
                <p className="settings-hint">Nhập API Key từ console.groq.com</p>
                <input
                  type="password"
                  className="api-key-input"
                  placeholder="gsk_..."
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
              </div>
            </div>
          ) : (
            <>
              <div className="chat-messages">
                {allItems.length === 0 ? (
                  <div className="welcome-section">
                    <div className="welcome-icon">
                      <img src={aiAgentIcon} alt="Welcome" />
                    </div>
                    <strong>Xin chào! Tôi là K2 Assistant</strong>
                    <p>Tôi có thể giúp bạn:</p>
                    <ul style={{ textAlign: 'left', paddingLeft: 16, color: '#9d9d9d', fontSize: 12, marginBottom: 12 }}>
                      <li>Tìm kiếm sản phẩm P2P</li>
                      <li>Tạo yêu cầu mua bán mới</li>
                      <li>Đàm phán giá tự động</li>
                    </ul>
                    <div className="suggestions">
                      {DEFAULT_SUGGESTIONS.map((s, i) => (
                        <button key={i} className="suggestion-btn" onClick={() => setInputValue(s)}>
                          {s}
                        </button>
                      ))}
                    </div>
                  </div>
                ) : (
                  allItems.map((item) => {
                    if (item.type === 'chatMessage') {
                      const msg = item.data;
                      return (
                        <div key={item.id} className={`message ${msg.role}`}>
                          {msg.role === 'assistant' && (
                            <img src={aiAgentIcon} alt="AI" className="ai-agent-large message-avatar" style={{ width: 28, height: 28, flexShrink: 0 }} />
                          )}
                          <div className="message-content">
                            <MarkdownContent content={msg.content} />
                          </div>
                        </div>
                      );
                    } else if (item.type === 'startButton') {
                      return (
                        <div key={item.id} className="message assistant">
                          <img src={aiAgentIcon} alt="AI" className="ai-agent-large message-avatar" style={{ width: 28, height: 28, flexShrink: 0 }} />
                          <div className="message-content">
                            <StartTransactionButton actionText={item.data.actionText} title={item.data.title} />
                          </div>
                        </div>
                      );
                    } else if (item.type === 'negotiationResult') {
                      return (
                        <div key={item.id} className="message assistant">
                          <img src={aiAgentIcon} alt="AI" className="ai-agent-large message-avatar" style={{ width: 28, height: 28, flexShrink: 0 }} />
                          <div className="message-content">
                            <MarkdownContent content={item.data} />
                            <p style={{ marginTop: 12, fontSize: 12, color: '#9d9d9d' }}>
                              Bạn muốn tôi hỗ trợ thêm gì?
                            </p>
                          </div>
                        </div>
                      );
                    }
                    return null;
                  })
                )}

                {isProcessing && (
                  <div className="message assistant typing">
                    <img src={aiAgentIcon} alt="AI" className="message-avatar" style={{ width: 28, height: 28 }} />
                    <div className="message-content">
                      <div className="typing-indicator">
                        <span className="typing-dot"></span>
                        <span className="typing-dot"></span>
                        <span className="typing-dot"></span>
                        <span className="typing-text">Đang xử lý...</span>
                      </div>
                    </div>
                  </div>
                )}
                <div ref={messagesEndRef} />
              </div>

              <div className="chat-input-area">
                <textarea
                  className="chat-textarea"
                  placeholder="Nhập tin nhắn..."
                  rows={1}
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  onKeyDown={handleKeyPress}
                  disabled={isProcessing}
                />
                <button
                  className="chat-send-btn"
                  onClick={handleSubmit}
                  disabled={!inputValue.trim() || isProcessing}
                >
                  <span className="ant-btn-icon">
                    <svg stroke="currentColor" fill="currentColor" strokeWidth="0" viewBox="0 0 512 512" height="1em" width="1em" xmlns="http://www.w3.org/2000/svg">
                      <path d="m476.59 227.05-.16-.07L49.35 49.84A23.56 23.56 0 0 0 27.14 52 24.65 24.65 0 0 0 16 72.59v113.29a24 24 0 0 0 19.52 23.57l232.93 43.07a4 4 0 0 1 0 7.86L35.53 303.45A24 24 0 0 0 16 327v113.31A23.57 23.57 0 0 0 26.59 460a23.94 23.94 0 0 0 13.22 4 24.55 24.55 0 0 0 9.52-1.93L476.4 285.94l.19-.09a32 32 0 0 0 0-58.8z"></path>
                    </svg>
                  </span>
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </>
  );
};

export default ChatInterface;
