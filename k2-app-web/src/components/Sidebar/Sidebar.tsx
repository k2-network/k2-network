import { NavLink } from "react-router-dom";
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
    badges?: Partial<Record<TabType, number>>;
}

const tabs: { id: TabType; icon: string; label: string }[] = [
    { id: "marketplace", icon: marketplaceIcon, label: "Marketplace" },
    { id: "negotiation", icon: negotiationIcon, label: "Negotiation" },
    { id: "contact", icon: contactIcon, label: "Contact" },
    { id: "profile", icon: profileIcon, label: "Profile" },
];

export function Sidebar({ activeTab: _activeTab, onTabChange, badges = {} }: SidebarProps) {
    return (
        <aside className="sidebar">
            <div className="sidebar-logo">
                <div className="logo-icon"><img src={k2Logo} alt="User" /></div>
            </div>

            <nav className="sidebar-nav">
                {tabs.map((tab) => {
                    const count = badges[tab.id] ?? 0;
                    return (
                        <NavLink
                            key={tab.id}
                            to={`/${tab.id}`}
                            className={({ isActive }) => `sidebar-item${isActive ? " active" : ""}${count > 0 ? " has-badge" : ""}`}
                            title={tab.label}
                            onClick={() => onTabChange(tab.id)}
                        >
                            <span className="sidebar-icon">
                                <img src={tab.icon} alt={tab.label} />
                                {count > 0 && (
                                    <span className="sidebar-badge">
                                        {count > 99 ? "99+" : count}
                                    </span>
                                )}
                            </span>
                        </NavLink>
                    );
                })}
            </nav>
        </aside>
    );
}
