import { TabType } from "../../types";
import "./Sidebar.css";
import k2Logo from "../../assets/icons/k2-logo.svg";

import marketplaceIcon from "../../assets/icons/marketplace.svg";
import negotiationIcon from "../../assets/icons/negotiation.svg";
import contactIcon from "../../assets/icons/contact.svg";
import profileIcon from "../../assets/icons/profile.svg";

interface SidebarProps {
    activeTab: TabType;
    onTabChange: (tab: TabType) => void;
}

const tabs: { id: TabType; icon: string; label: string }[] = [
    { id: "marketplace", icon: marketplaceIcon, label: "Marketplace" },
    { id: "negotiation", icon: negotiationIcon, label: "Negotiation" },
    { id: "contact", icon: contactIcon, label: "Contact" },
    { id: "profile", icon: profileIcon, label: "Profile" },
];

export function Sidebar({ activeTab, onTabChange }: SidebarProps) {
    return (
        <aside className="sidebar">
            <div className="sidebar-logo">
                <div className="logo-icon"><img src={k2Logo} alt="User" /></div>
            </div>

            <nav className="sidebar-nav">
                {tabs.map((tab) => (
                    <button
                        key={tab.id}
                        className={`sidebar-item ${activeTab === tab.id ? "active" : ""}`}
                        onClick={() => onTabChange(tab.id)}
                        title={tab.label}
                    >
                        <span className="sidebar-icon">
                            <img src={tab.icon} alt={tab.label} />
                        </span>
                    </button>
                ))}
            </nav>
        </aside>
    );
}
