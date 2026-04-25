import { IoCopy, IoCheckmark, IoSparkles, IoTrophy, IoFlash } from "react-icons/io5";
import { useState } from "react";
import { ProfileData } from "./types";
import "./Profile.css";

interface ProfileViewerProps {
    data: ProfileData;
}

export function ProfileViewer({ data }: ProfileViewerProps) {
    const [copied, setCopied] = useState(false);

    const copyNodeId = () => {
        navigator.clipboard.writeText(data.nodeId);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const shortNodeId = data.nodeId ? `${data.nodeId.slice(0, 12)}...${data.nodeId.slice(-8)}` : "Loading...";

    return (
        <div className="profile-page viewer-mode">
            {/* Left Panel */}
            <div className="profile-left">
                <div className="profile-header">
                    <div className="profile-avatar">
                        {data.avatar ? (
                            <img src={data.avatar} alt="Avatar" className="avatar-img" />
                        ) : (
                            <span>{data.username.charAt(0).toUpperCase()}</span>
                        )}
                    </div>
                    <div className="profile-name-section">
                        <div className="profile-name-container">
                            <h2 className="profile-name">
                                {data.username} <IoSparkles className="name-icon" />
                            </h2>
                        </div>
                        <div className="profile-stats-row">
                            <span className="profile-connections">{data.connections} Connections</span>
                        </div>
                        {data.intro && (
                            <div className="profile-intro-display">
                                {data.intro}
                            </div>
                        )}
                    </div>
                </div>

                <div className="profile-badges">
                    <span className="badge badge-primary"><IoTrophy /> TOP 1</span>
                    <span className="badge badge-secondary"><IoFlash /> CONTRIBUTOR </span>
                </div>

                <div className="node-id-section">
                    <div className="node-id-label">Node ID</div>
                    <div className="node-id-box">
                        <code className="node-id-text">{shortNodeId}</code>
                        <button className={`copy-btn ${copied ? 'copied' : ''}`} onClick={copyNodeId}>
                            {copied ? <IoCheckmark /> : <IoCopy />}
                        </button>
                    </div>
                </div>
            </div>

            {/* Right Panel */}
            <div className="profile-right">
                <div className="logo-upload-grid">
                    <div className="logo-display-card">
                        <div className="logo-preview empty">
                            {data.logoDark ? (
                                <img src={data.logoDark} alt="Logo Dark" className="logo-img" />
                            ) : (
                                <IoFlash className="upload-icon" />
                            )}
                        </div>
                    </div>
                </div>

                <div className="description-section">
                    <div className="description-label">DESCRIPTION</div>
                    <div className="profile-description-display">
                        {data.description || "No description provided."}
                    </div>
                </div>

                {/* Activity Timeline */}
                <div className="activity-section">
                    <div className="section-header">
                        <span className="section-title">RECENT ACTIVITY</span>
                    </div>
                    <div className="activity-list">
                        <div className="activity-item">
                            <div className="activity-dot" />
                            <div className="activity-content">
                                <span className="activity-text">Synced document with <strong>BobRelay</strong></span>
                                <span className="activity-time">2 minutes ago</span>
                            </div>
                        </div>
                        <div className="activity-item">
                            <div className="activity-dot" />
                            <div className="activity-content">
                                <span className="activity-text">Connected to <strong>AliceNode</strong></span>
                                <span className="activity-time">15 minutes ago</span>
                            </div>
                        </div>
                        <div className="activity-item">
                            <div className="activity-dot" />
                            <div className="activity-content">
                                <span className="activity-text">Shared 128MB blob data</span>
                                <span className="activity-time">1 hour ago</span>
                            </div>
                        </div>
                        <div className="activity-item">
                            <div className="activity-dot" />
                            <div className="activity-content">
                                <span className="activity-text">Node started successfully</span>
                                <span className="activity-time">14 days ago</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
