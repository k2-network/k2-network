import "./Header.css";
import accountCircleIcon from "../../assets/icons/account-circle.svg";

interface HeaderProps {
    title: string;
    nodeId?: string;
    teamName?: string;
}

export function Header({ title, nodeId, teamName = "k2-team" }: HeaderProps) {
    const shortNodeId = nodeId ? `NodeId: ${nodeId.slice(0, 10)}...` : "";

    return (
        <header className="header-bar">
            <h1 className="header-title">{title}</h1>

            <div className="header-right">
                <div className="header-user">
                    <div className="user-icon">
                        <img src={accountCircleIcon} alt="User" />
                    </div>
                    <span className="team-name">{teamName}</span>
                </div>
                <div className="header-divider" />
                <span className="node-id">{shortNodeId}</span>
            </div>
        </header>
    );
}
