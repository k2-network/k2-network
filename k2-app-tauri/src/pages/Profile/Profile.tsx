import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IoCopy, IoCheckmark, IoSparkles, IoTrophy, IoFlash, IoEye } from "react-icons/io5";
import { ProfileViewer } from "./ProfileViewer";
import { ProfileData } from "./types";
import "./Profile.css";

export function ProfilePage() {
    const [nodeId, setNodeId] = useState<string>("");
    const [copied, setCopied] = useState(false);
    const [isEditMode, setIsEditMode] = useState(false);
    const [isPreviewMode, setIsPreviewMode] = useState(false);
    
    // Profile Data States
    const [username, setUsername] = useState('Anonymous');
    const [intro, setIntro] = useState('');
    const [description, setDescription] = useState('');
    const [avatar, setAvatar] = useState<string | null>(null);
    const [logoDark, setLogoDark] = useState<string | null>(null);
    const [logoLight, setLogoLight] = useState<string | null>(null);

    const [isEditingName, setIsEditingName] = useState(false);

    // Refs for hidden inputs
    const avatarInputRef = useRef<HTMLInputElement>(null);
    const logoDarkInputRef = useRef<HTMLInputElement>(null);
    const logoLightInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        const fetchProfile = async () => {
            try {
                // Get node ID
                const id = await invoke<string>('get_my_node_id');
                setNodeId(id);

                // Get profile data
                const p = await invoke<any>('get_profile');
                setUsername(p.name || 'Anonymous');
                setIntro(p.intro || '');
                setDescription(p.description || '');
                
                // Fetch images if hashes exist
                if (p.avatar_hash) {
                    const b64 = await invoke<string>('get_profile_image', { hash: p.avatar_hash });
                    setAvatar(b64);
                }
                if (p.logo_hash) {
                    const b64 = await invoke<string>('get_profile_image', { hash: p.logo_hash });
                    setLogoDark(b64);
                }
                if (p.logo_light_hash) {
                    const b64 = await invoke<string>('get_profile_image', { hash: p.logo_light_hash });
                    setLogoLight(b64);
                }
            } catch (err) {
                console.error('Failed to fetch profile:', err);
            }
        };
        fetchProfile();
    }, []);

    const copyNodeId = () => {
        if (!nodeId) return;
        navigator.clipboard.writeText(nodeId);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const handleImageUpload = async (e: React.ChangeEvent<HTMLInputElement>, type: 'avatar' | 'logoDark' | 'logoLight') => {
        const file = e.target.files?.[0];
        if (file) {
            try {
                // Read as ArrayBuffer for Rust
                const arrayBuffer = await file.arrayBuffer();
                const bytes = Array.from(new Uint8Array(arrayBuffer));
                
                // Map frontend type to backend field name
                const field = type === 'avatar' ? 'avatar' : (type === 'logoDark' ? 'logo_dark' : 'logo_light');
                
                // Upload to Iroh Blobs
                await invoke('update_profile_image', { field, bytes });

                // Read as DataURL for immediate UI preview
                const reader = new FileReader();
                reader.onloadend = () => {
                    const base64String = reader.result as string;
                    if (type === 'avatar') setAvatar(base64String);
                    else if (type === 'logoDark') setLogoDark(base64String);
                    else if (type === 'logoLight') setLogoLight(base64String);
                };
                reader.readAsDataURL(file);
            } catch (err) {
                console.error('Failed to upload image:', err);
            }
        }
    };

    const handleRemoveImage = (type: 'avatar' | 'logoDark' | 'logoLight') => {
        // Logic to remove from docs (optional, just clearing local state for now)
        if (type === 'avatar') setAvatar(null);
        else if (type === 'logoDark') setLogoDark(null);
        else if (type === 'logoLight') setLogoLight(null);
    };

    const profileData: ProfileData = {
        username,
        intro,
        connections: 128, // Mock
        nodeId,
        avatar,
        logoDark,
        logoLight,
        description
    };

    if (!isEditMode) {
        return (
            <div className="profile-viewer-wrapper">
                {!isPreviewMode && (
                    <div className="viewer-controls">
                        <button className="edit-profile-btn" onClick={() => setIsEditMode(true)}>
                            Edit Profile
                        </button>
                    </div>
                )}
                {isPreviewMode && (
                    <div className="preview-toolbar">
                        <span className="preview-mode-hint">Preview Mode</span>
                        <button className="exit-preview-btn" onClick={() => setIsPreviewMode(false)}>
                            Exit Preview
                        </button>
                    </div>
                )}
                <ProfileViewer data={profileData} />
            </div>
        );
    }

    const shortNodeId = nodeId ? `${nodeId.slice(0, 12)}...${nodeId.slice(-8)}` : "Loading...";

    return (
        <div className="profile-page">
            {/* Left Panel */}
            <div className="profile-left">
                {/* Header */}
                <div className="profile-header">
                    <div className="profile-avatar" onClick={() => avatarInputRef.current?.click()} title="Change avatar">
                        {avatar ? (
                            <img src={avatar} alt="Avatar" className="avatar-img" />
                        ) : (
                            <span>{username.charAt(0).toUpperCase()}</span>
                        )}
                        <input 
                            type="file" 
                            ref={avatarInputRef} 
                            style={{ display: 'none' }} 
                            accept="image/*"
                            onChange={(e) => handleImageUpload(e, 'avatar')}
                        />
                    </div>
                    <div className="profile-name-section">
                        <div className="profile-name-container">
                            {isEditingName ? (
                                <input
                                    className="profile-name-input"
                                    defaultValue={username}
                                    autoFocus
                                    onBlur={(e) => {
                                        const val = e.target.value.trim() || 'Anonymous';
                                        setUsername(val);
                                        localStorage.setItem('k2_username', val);
                                        setIsEditingName(false);
                                    }}
                                    onKeyDown={(e) => {
                                        if (e.key === 'Enter') {
                                            const val = e.currentTarget.value.trim() || 'Anonymous';
                                            setUsername(val);
                                            localStorage.setItem('k2_username', val);
                                            setIsEditingName(false);
                                        }
                                        if (e.key === 'Escape') setIsEditingName(false);
                                    }}
                                />
                            ) : (
                                <h2 className="profile-name">
                                    {username} <IoSparkles className="name-icon" />
                                </h2>
                            )}
                            <div className="editor-controls">
                                <button className="save-profile-btn" onClick={async () => {
                                    try {
                                        await invoke('update_profile_text', { 
                                            name: username, 
                                            intro, 
                                            description 
                                        });
                                        setIsEditMode(false);
                                    } catch (err) {
                                        console.error('Failed to save profile:', err);
                                    }
                                }}>
                                    Save & View
                                </button>
                                <button className="view-as-btn" title="View as others" onClick={async () => {
                                    try {
                                        await invoke('update_profile_text', { 
                                            name: username, 
                                            intro, 
                                            description 
                                        });
                                        setIsPreviewMode(true);
                                        setIsEditMode(false);
                                    } catch (err) {
                                        console.error('Failed to save profile:', err);
                                    }
                                }}>
                                    <IoEye />
                                </button>
                            </div>
                        </div>
                        <div className="profile-stats-row">
                            <span className="profile-connections">128 Connections</span>
                        </div>
                        <div className="profile-intro-section">
                            <input 
                                className="profile-intro-input" 
                                placeholder="Write a short intro about yourself..."
                                value={intro}
                                onChange={(e) => setIntro(e.target.value)}
                            />
                        </div>
                    </div>
                </div>

                {/* Badges */}
                <div className="profile-badges">
                    <span className="badge badge-primary"><IoTrophy /> TOP 1</span>
                    <span className="badge badge-secondary"><IoFlash /> CONTRIBUTOR </span>
                </div>

                <div className="node-id-section">
                    <div className="node-id-label">Your Node ID</div>
                    <div className="node-id-box">
                        <code className="node-id-text">{shortNodeId}</code>
                        <button className={`copy-btn ${copied ? 'copied' : ''}`} onClick={copyNodeId} title={copied ? 'Copied!' : 'Copy'}>
                            {copied ? <IoCheckmark /> : <IoCopy />}
                        </button>
                    </div>
                </div>
            </div>

            {/* Right Panel - Branding */}
            <div className="profile-right">
                <div className="branding-title">BRANDING & LOGO</div>

                <div className="logo-upload-grid">
                    <div className="logo-upload-card">
                        <div className="logo-preview empty">
                            {logoDark ? (
                                <img src={logoDark} alt="Logo Dark" className="logo-img" />
                            ) : (
                                <IoFlash className="upload-icon" />
                            )}
                        </div>
                        <div className="logo-info">
                            <span className="logo-label">DARK MODE</span>
                            <div className="logo-actions">
                                <button className="upload-btn" onClick={() => logoDarkInputRef.current?.click()}>
                                    {logoDark ? 'Change' : 'Upload'}
                                </button>
                                {logoDark && (
                                    <button className="remove-btn" onClick={() => handleRemoveImage('logoDark')}>
                                        Remove
                                    </button>
                                )}
                            </div>
                            <input 
                                type="file" 
                                ref={logoDarkInputRef} 
                                style={{ display: 'none' }} 
                                accept="image/*"
                                onChange={(e) => handleImageUpload(e, 'logoDark')}
                            />
                        </div>
                    </div>

                    <div className="logo-upload-card">
                        <div className="logo-preview empty light">
                            {logoLight ? (
                                <img src={logoLight} alt="Logo Light" className="logo-img" />
                            ) : (
                                <IoFlash className="upload-icon" />
                            )}
                        </div>
                        <div className="logo-info">
                            <span className="logo-label">LIGHT MODE</span>
                            <div className="logo-actions">
                                <button className="upload-btn" onClick={() => logoLightInputRef.current?.click()}>
                                    {logoLight ? 'Change' : 'Upload'}
                                </button>
                                {logoLight && (
                                    <button className="remove-btn" onClick={() => handleRemoveImage('logoLight')}>
                                        Remove
                                    </button>
                                )}
                            </div>
                            <input 
                                type="file" 
                                ref={logoLightInputRef} 
                                style={{ display: 'none' }} 
                                accept="image/*"
                                onChange={(e) => handleImageUpload(e, 'logoLight')}
                            />
                        </div>
                    </div>
                </div>

                <div className="description-section">
                    <div className="description-label">DESCRIPTION</div>
                    <textarea 
                        className="profile-description-area" 
                        placeholder="Describe your node or service in detail..."
                        value={description}
                        onChange={(e) => setDescription(e.target.value)}
                    />
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
