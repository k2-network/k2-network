import "./Header.css";
import accountCircleIcon from "../../assets/icons/account-circle.svg";
import aiAgentIcon from "../../assets/icons/ai-agent.svg";
import { ThemeSwitcher } from "../ThemeSwitcher/ThemeSwitcher";

interface HeaderProps {
    title: string;
    teamName?: string;
    onLogout?: () => void;
    onOpenAIChat?: () => void;
    isAIChatOpen?: boolean;
}

export function Header({ title, teamName = "k2-team", onLogout, onOpenAIChat, isAIChatOpen }: HeaderProps) {
    return (
        <header className="header-bar">
            <div className="header-left" />
            <h1 className="header-title">{title}</h1>

            <div className="header-right">
                {onOpenAIChat && (
                    <button
                        className={`ai-chat-btn ${isAIChatOpen ? 'ai-chat-btn--active' : ''}`}
                        onClick={onOpenAIChat}
                        title="K2 AI Assistant"
                    >
                        <span className="ai-chat-btn__glow" />
                        <img src={aiAgentIcon} alt="AI" className="ai-chat-btn__icon" />
                        <span className="ai-chat-btn__label">AI Lab</span>
                    </button>
                )}
                <ThemeSwitcher />
                <div className="header-divider" />
                <div className="header-user">
                    <div className="user-icon">
                        <img src={accountCircleIcon} alt="User" />
                    </div>
                    <span className="team-name">{teamName}</span>
                </div>
                {onLogout && (
                    <>
                        <div className="header-divider" />
                        <button className="header-logout-btn" onClick={onLogout}>
                            Đăng xuất
                        </button>
                    </>
                )}
            </div>
        </header>
    );
}
