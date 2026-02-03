import React, { useState, useRef, useEffect, useCallback } from 'react';
import { IoClose, IoSettingsOutline } from 'react-icons/io5';
import ReactMarkdown from 'react-markdown';
import {
  TamboProvider,
  useTamboThread,
  useTamboThreadInput,
  GenerationStage,
} from "@tambo-ai/react";
import { TAMBO_API_KEY, tamboTools, tamboComponents } from '../../tambo';
import './ChatInterface.css';
import aiAgentIconLight from '../../assets/icons/ai-agent-large.svg';
import aiAgentIconDark from '../../assets/icons/ai-agent-large-dark.svg';

const DEFAULT_SUGGESTIONS = [
  "Tôi muốn mua video",
  "Bán dịch vụ thiết kế UI/UX",
  "Trao đổi laptop cũ",
  "Giới thiệu về K2 Marketplace"
];

// Markdown renderer component - Dark Theme
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
              <code style={{
                background: '#3c3c3c',
                padding: '2px 6px',
                borderRadius: 3,
                fontSize: '0.9em',
                color: '#ce9178'
              }}>
                {children}
              </code>
            ) : (
              <pre style={{
                background: '#1a1a1a',
                padding: 10,
                borderRadius: 6,
                overflow: 'auto',
                fontSize: '0.85em',
                border: '1px solid #3c3c3c'
              }}>
                <code style={{ color: '#d4d4d4' }}>{children}</code>
              </pre>
            );
          },
          table: ({ children }) => (
            <table style={{
              borderCollapse: 'collapse',
              width: '100%',
              marginBottom: 10,
              fontSize: '0.85em'
            }}>
              {children}
            </table>
          ),
          th: ({ children }) => (
            <th style={{
              border: '1px solid #3c3c3c',
              padding: '6px 10px',
              background: '#2d2d30',
              textAlign: 'left',
              color: '#cccccc'
            }}>
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td style={{ border: '1px solid #3c3c3c', padding: '6px 10px', color: '#cccccc' }}>
              {children}
            </td>
          ),
          strong: ({ children }) => <strong style={{ fontWeight: 600, color: '#e0e0e0' }}>{children}</strong>,
          blockquote: ({ children }) => (
            <blockquote style={{
              borderLeft: '3px solid #0078d4',
              paddingLeft: 12,
              margin: '8px 0',
              color: '#9d9d9d'
            }}>
              {children}
            </blockquote>
          ),
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

// Inner component that uses Tambo hooks
const ChatContent: React.FC<ChatInterfaceProps> = ({ isOpen, onToggle, width, onWidthChange }) => {
  const [showSettings, setShowSettings] = useState(false);
  const [theme, setTheme] = useState<'dark' | 'light'>(() => {
    const saved = localStorage.getItem('k2-chat-theme');
    return (saved as 'dark' | 'light') || 'dark';
  });
  const [isResizing, setIsResizing] = useState(false);
  const [groqApiKey, setGroqApiKey] = useState<string>(() => {
    return localStorage.getItem('groq-api-key') || import.meta.env.VITE_GROQ_API_KEY || '';
  });

  // Save Groq API key to localStorage
  const handleGroqApiKeyChange = (value: string) => {
    setGroqApiKey(value);
    localStorage.setItem('groq-api-key', value);
  };

  // Tambo hooks
  const { thread, generationStage } = useTamboThread();
  const { value: inputValue, setValue: setInputValue, submit, isPending } = useTamboThreadInput();

  const messages = thread?.messages || [];
  const isProcessing = isPending || (generationStage !== GenerationStage.IDLE && generationStage !== GenerationStage.COMPLETE);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  // Parse form data from AI response and trigger Marketplace form
  useEffect(() => {
    if (messages.length > 0) {
      const lastMessage = messages[messages.length - 1];
      if (lastMessage.role === 'assistant' && Array.isArray(lastMessage.content)) {
        // Look for form data in text content
        for (const block of lastMessage.content) {
          if ((block as any).type === 'text') {
            const text = (block as any).text || '';
            // Check for form data markers
            const formDataMatch = text.match(/<!-- FORM_DATA_START -->\s*([\s\S]*?)\s*<!-- FORM_DATA_END -->/);
            if (formDataMatch) {
              try {
                const formData = JSON.parse(formDataMatch[1]);
                console.log("📋 [ChatInterface] Extracted form data:", formData);
                // Dispatch event to Marketplace
                window.dispatchEvent(new CustomEvent('k2:showDynamicForm', {
                  detail: { data: formData, streaming: false }
                }));
              } catch (err) {
                console.error("Failed to parse form data:", err);
              }
            }
          }
        }
      }
    }
  }, [messages]);

  // Handle resize
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
  }, []);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;
      const newWidth = window.innerWidth - e.clientX;
      // Min 280px, max 600px
      if (newWidth >= 280 && newWidth <= 600) {
        onWidthChange(newWidth);
      }
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

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
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isProcessing]);

  const handleSubmit = async () => {
    if (!inputValue.trim() || isProcessing) return;
    await submit();
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleSuggestionClick = (suggestion: string) => {
    setInputValue(suggestion);
  };

  // Select icon based on theme
  const aiAgentIcon = theme === 'dark' ? aiAgentIconDark : aiAgentIconLight;

  return (
    <>
      {/* Floating Action Button - only show when closed */}
      {!isOpen && (
        <button
          className="chat-fab-button"
          onClick={onToggle}
          style={{ backgroundImage: `url(${aiAgentIcon})` }}
          aria-label="Open AI Assistant"
        />
      )}

      {/* Chat Panel - Integrated Sidebar */}
      {isOpen && (
        <div
          ref={panelRef}
          className={`chat-panel ${theme}`}
          style={{ width: `${width}px` }}
        >
          {/* Resize Handle */}
          <div
            className="chat-resize-handle"
            onMouseDown={handleMouseDown}
          />

          {/* Header */}
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

          {/* Settings Panel */}
          {showSettings ? (
            <div className="api-key-overlay">
              {/* <h4 style={{ marginBottom: 16, color: '#cccccc' }}>Cài đặt</h4> */}

              {/* Theme Toggle */}
              {/* <div className="settings-section">
                <label className="settings-label">Giao diện</label>
                <div className="theme-toggle-container">
                  <button
                    className={`theme-btn ${theme === 'light' ? 'active' : ''}`}
                    onClick={() => { setTheme('light'); localStorage.setItem('k2-chat-theme', 'light'); }}
                  >
                    Light
                  </button>
                  <button
                    className={`theme-btn ${theme === 'dark' ? 'active' : ''}`}
                    onClick={() => { setTheme('dark'); localStorage.setItem('k2-chat-theme', 'dark'); }}
                  >
                    Dark
                  </button>
                </div>
              </div> */}

              {/* Groq API Key Section */}
              <div className="settings-section">
                <label className="settings-label" >Groq API Key</label>
                <p className="settings-hint" >
                  Nhập API Key từ console.groq.com (cho structured output)
                </p>
                <input
                  type="password"
                  className="api-key-input"
                  placeholder="gsk_..."
                  value={groqApiKey}
                  onChange={(e) => handleGroqApiKeyChange(e.target.value)}
                />
                {/* {groqApiKey && (
                  <p className="settings-hint" style={{ color: '#4ec9b0', marginTop: 4 }}>
                    ✓ API Key đã được lưu
                  </p>
                )} */}
              </div>

              {/* <button
                className="settings-save-btn"
                onClick={() => setShowSettings(false)}
              >
                Đóng cài đặt
              </button> */}
            </div>
          ) : (
            <>
              {/* Messages Area */}
              <div className="chat-messages">
                {messages.length === 0 ? (
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
                        <button key={i} className="suggestion-btn" onClick={() => handleSuggestionClick(s)}>
                          {s}
                        </button>
                      ))}
                    </div>
                  </div>
                ) : (
                  messages.map((msg: any) => {
                    // Tambo messages have content as array of { type, text }
                    const content = typeof msg.content === 'string'
                      ? msg.content
                      : Array.isArray(msg.content)
                        ? msg.content
                          .filter((c: any) => c.type === 'text')
                          .map((c: any) => c.text || '')
                          .join('\n')
                        : '';

                    // Check if message has a rendered component
                    const hasRenderedComponent = msg.renderedComponent !== undefined && msg.renderedComponent !== null;

                    return (
                      <div key={msg.id} className={`message ${msg.role}`}>
                        {msg.role === 'assistant' && (
                          <img src={aiAgentIcon} alt="AI" className="ai-agent-large message-avatar" style={{ width: 28, height: 28, flexShrink: 0 }} />
                        )}
                        <div className="message-content">
                          {/* Render text content */}
                          {content && <MarkdownContent content={content} />}

                          {/* Render Tambo generated component */}
                          {hasRenderedComponent && (
                            <div className="generated-component-wrapper">
                              {msg.renderedComponent}
                            </div>
                          )}
                        </div>
                      </div>
                    );
                  })
                )}


                {isProcessing && (
                  <div className="message assistant typing">
                    <img src={aiAgentIcon} alt="AI" className="ai-agent-large message-avatar" style={{ width: 28, height: 28 }} />
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

              {/* Input Area */}
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

// Main export - wraps ChatContent with TamboProvider
const ChatInterface: React.FC<ChatInterfaceProps> = (props) => {
  return (
    <TamboProvider
      apiKey={TAMBO_API_KEY}
      components={tamboComponents}
      tools={tamboTools}
      contextKey="k2-marketplace"
    >
      <ChatContent {...props} />
    </TamboProvider>
  );
};

export default ChatInterface;
