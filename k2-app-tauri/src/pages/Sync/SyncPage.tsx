import React, { useState, useEffect, useRef } from "react";
import "./SyncPage.css";
import fragmentIcon from "../../assets/icons/fragment.svg";
import k2Logo from "../../assets/icons/k2-logo.svg";
import syncLogo from "../../assets/sync-logo.svg";
import folderIcon from "../../assets/icons/folder.svg";
import compassIcon from "../../assets/icons/compass_calibration.svg";
import aiAgentLogo from "../../assets/icons/ai-agent-large-dark.svg";

import { invoke } from "@tauri-apps/api/core";

interface FolderInfo {
    id: string;
    name: string;
    path: string;
    syncInterval: number;
    syncMode: 'proactive' | 'passive';
    syncEnabled: boolean;
    linkedDevices: string[];
    lastScan?: string; // Optional for UI display
}

interface DeviceInfo {
    id: string;
    name: string;
    deviceType: string;
    nodeId: string;
    status?: "online" | "offline";
}

interface SyncSettings {
    localLogo?: string;
    displayName?: string;
}

export function SyncPage() {
    const [folders, setFolders] = useState<FolderInfo[]>([]);
    const [devices, setDevices] = useState<DeviceInfo[]>([]);
    const [localLogo, setLocalLogo] = useState(syncLogo);
    const [selectedFolderId, setSelectedFolderId] = useState<string | null>(null);

    // Initial Load from iroh-docs via Tauri
    useEffect(() => {
        const loadAll = async () => {
            try {
                const [f, d, s] = await Promise.all([
                    invoke<FolderInfo[]>('get_sync_folders'),
                    invoke<DeviceInfo[]>('get_sync_devices'),
                    invoke<SyncSettings>('get_sync_settings')
                ]);
                setFolders(f);
                setDevices(d);
                if (s.localLogo) setLocalLogo(s.localLogo);
                console.log("[Sync] Loaded config from iroh-docs");
            } catch (error) {
                console.error("[Sync] Failed to load config:", error);
            }
        };
        loadAll();
    }, []);

    // Helper to sync a single folder update to backend
    const syncFolderToBackend = async (folder: FolderInfo) => {
        try {
            await invoke('add_sync_folder', { config: folder });
        } catch (error) {
            console.error("[Sync] Failed to save folder:", error);
        }
    };

    // Helper to sync a single device update to backend
    const syncDeviceToBackend = async (device: DeviceInfo) => {
        try {
            await invoke('add_sync_device', { config: device });
        } catch (error) {
            console.error("[Sync] Failed to save device:", error);
        }
    };

    const fileInputRef = useRef<HTMLInputElement>(null);
    const [isEditing, setIsEditing] = useState(false);
    const [isAddDeviceOpen, setIsAddDeviceOpen] = useState(false);
    const [newNodeId, setNewNodeId] = useState("");
    const [newDeviceName, setNewDeviceName] = useState("");

    const selectedFolder = folders.find(f => f.id === selectedFolderId);

    const handleLocalLogoClick = () => {
        fileInputRef.current?.click();
    };

    const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0];
        if (file) {
            const reader = new FileReader();
            reader.onload = async (e) => {
                if (e.target?.result) {
                    const base64 = e.target.result as string;
                    setLocalLogo(base64);
                    try {
                        await invoke('update_sync_settings', { settings: { localLogo: base64 } });
                        console.log("[Sync] Logo updated in iroh-docs");
                    } catch (err) {
                        console.error("Failed to save logo:", err);
                    }
                }
            };
            reader.readAsDataURL(file);
        }
    };

    const toggleDeviceSelection = async (deviceId: string) => {
        if (!selectedFolderId || !isEditing) return;
        
        const folder = folders.find(f => f.id === selectedFolderId);
        if (!folder) return;

        const newDevices = folder.linkedDevices.includes(deviceId)
            ? folder.linkedDevices.filter(id => id !== deviceId)
            : [...folder.linkedDevices, deviceId];
            
        const updatedFolder = { ...folder, linkedDevices: newDevices };
        
        setFolders(prev => prev.map(f => f.id === selectedFolderId ? updatedFolder : f));
        await syncFolderToBackend(updatedFolder);
    };

    const updateFolderOption = async (key: keyof FolderInfo, value: any) => {
        if (!selectedFolderId) return;
        const folder = folders.find(f => f.id === selectedFolderId);
        if (!folder) return;

        const updatedFolder = { ...folder, [key]: value };
        setFolders(prev => prev.map(f => f.id === selectedFolderId ? updatedFolder : f));
        await syncFolderToBackend(updatedFolder);
    };

    const handleAddFolder = async () => {
        try {
            // @ts-ignore - Tauri API
            const selected = await window.__TAURI__.dialog.open({
                directory: true,
                multiple: false,
                title: 'Select Folder to Sync'
            });

            if (selected && typeof selected === 'string') {
                // Duplicate detection
                const isDuplicate = folders.some(f => f.path === selected);
                if (isDuplicate) {
                    alert("This folder is already in the sync list.");
                    return;
                }

                const newFolder: FolderInfo = {
                    id: `f-${Date.now()}`,
                    name: selected.split(/[\\/]/).pop() || "New Folder",
                    path: selected,
                    syncEnabled: true,
                    syncInterval: 10,
                    syncMode: 'proactive',
                    linkedDevices: []
                };
                setFolders([...folders, newFolder]);
                await syncFolderToBackend(newFolder);
                setSelectedFolderId(newFolder.id);
                console.log("[Sync] Added folder to iroh-docs:", newFolder);
            }
        } catch (error) {
            console.error("Failed to open folder dialog:", error);
        }
    };

    // Custom Confirm Dialog state
    const [confirmDialog, setConfirmDialog] = useState<{
        isOpen: boolean,
        title: string,
        message: string,
        actionType: 'danger' | 'info',
        onConfirm: () => void
    }>({
        isOpen: false,
        title: "",
        message: "",
        actionType: 'info',
        onConfirm: () => {}
    });

    const handleRemoveFolder = (id: string) => {
        setConfirmDialog({
            isOpen: true,
            title: "Remove Folder",
            message: "Are you sure you want to remove this folder and its configuration from the sync list?",
            actionType: 'danger',
            onConfirm: async () => {
                try {
                    await invoke('remove_sync_folder', { id });
                    setFolders(prev => prev.filter(f => f.id !== id));
                    if (selectedFolderId === id) setSelectedFolderId(null);
                    setConfirmDialog(prev => ({ ...prev, isOpen: false }));
                    console.log("[Sync] Removed folder from iroh-docs:", id);
                } catch (error) {
                    console.error("Failed to remove folder:", error);
                }
            }
        });
    };

    const handleRemoveDevice = (id: string) => {
        setConfirmDialog({
            isOpen: true,
            title: "Remove Device",
            message: "Are you sure you want to remove this device from your list?",
            actionType: 'danger',
            onConfirm: async () => {
                try {
                    // 1. Remove device from DB
                    await invoke('remove_sync_device', { id });
                    
                    // 2. Filter device from all folders
                    const updatedFolders = folders.map(f => ({
                        ...f,
                        linkedDevices: f.linkedDevices.filter(dId => dId !== id)
                    }));

                    // 3. Persist changed folders
                    for (const folder of updatedFolders) {
                        const original = folders.find(of => of.id === folder.id);
                        if (original && original.linkedDevices.length !== folder.linkedDevices.length) {
                            await syncFolderToBackend(folder);
                        }
                    }

                    // 4. Update UI state
                    setDevices(prev => prev.filter(d => d.id !== id));
                    setFolders(updatedFolders);
                    setConfirmDialog(prev => ({ ...prev, isOpen: false }));
                    console.log("[Sync] Removed device and cleaned up folders:", id);
                } catch (error) {
                    console.error("Failed to remove device:", error);
                }
            }
        });
    };

    const handleAddDevice = async () => {
        if (!newNodeId || !newDeviceName) return;
        const newDevice: DeviceInfo = {
            id: `dev-${Date.now()}`,
            name: newDeviceName,
            deviceType: "Desktop",
            status: "offline",
            nodeId: newNodeId
        };
        setDevices([...devices, newDevice]);
        await syncDeviceToBackend(newDevice);
        setIsAddDeviceOpen(false);
        setNewNodeId("");
        setNewDeviceName("");
        console.log("[Sync] Registered device to iroh-docs:", newDevice);
    };

    return (
        <div className="sync-page-v2">
            <div className="sync-layout-v2">
                {/* Local Folder Column */}
                <div className="sync-column local">
                    <input type="file" ref={fileInputRef} style={{ display: 'none' }} accept="image/*" onChange={handleFileChange} />
                    <div className="sync-column-header">
                        <div className="sync-header-logo-circle" onClick={handleLocalLogoClick} title="Click to upload logo">
                            <img src={localLogo} alt="Sync" className="sync-logo-img" />
                        </div>
                    </div>
                    <div className="sync-column-content">
                        <div className="sync-content-title">
                            FOLDERS
                            <button className="sync-add-btn-small" onClick={handleAddFolder} title="Add Folder">+</button>
                        </div>
                        <div className="sync-item-list">
                            {folders.map(f => (
                                <div 
                                    key={f.id} 
                                    className={`sync-list-item ${selectedFolderId === f.id ? 'active' : ''}`}
                                    onClick={() => { 
                                        if (selectedFolderId === f.id) {
                                            setSelectedFolderId(null);
                                        } else {
                                            setSelectedFolderId(f.id); 
                                            setIsEditing(false); 
                                        }
                                    }}
                                >
                                    <div className="sync-item-left">
                                        <img src={folderIcon} className="sync-folder-icon" alt="" />
                                        <span className="sync-item-name">{f.name}</span>
                                    </div>
                                    <div className={`sync-item-status-dot ${f.syncEnabled ? 'syncing' : 'paused'}`}></div>
                                </div>
                            ))}
                        </div>
                    </div>
                </div>

                {/* Connection Section */}
                <div className="sync-bridge-v2">
                    <div className="sync-bridge-lines top">
                        <div className="sync-line ltr"><div className="sync-particle sync-p-ltr"></div></div>
                        <div className="sync-line ltr"><div className="sync-particle sync-p-ltr" style={{animationDelay: '0.5s'}}></div></div>
                        <div className="sync-line ltr"><div className="sync-particle sync-p-ltr" style={{animationDelay: '1s'}}></div></div>
                    </div>

                    <div className="sync-bridge-center">
                        <div className="sync-fragment-loader">
                            <img src={fragmentIcon} className="sync-frag sync-f1" alt="" />
                            <img src={fragmentIcon} className="sync-frag sync-f2" alt="" />
                            <img src={fragmentIcon} className="sync-frag sync-f3" alt="" />
                            <img src={fragmentIcon} className="sync-frag sync-f4" alt="" />
                        </div>
                        <div className="sync-bridge-status">Sync is in progress...</div>
                    </div>

                    <div className="sync-bridge-lines bottom">
                        <div className="sync-line rtl"><div className="sync-particle sync-p-rtl"></div></div>
                        <div className="sync-line rtl"><div className="sync-particle sync-p-rtl" style={{animationDelay: '0.7s'}}></div></div>
                        <div className="sync-line rtl"><div className="sync-particle sync-p-rtl" style={{animationDelay: '1.4s'}}></div></div>
                    </div>
                </div>

                {/* Remote Devices Column */}
                <div className="sync-column remote">
                    <div className="sync-column-header">
                        <div className="sync-header-logo-circle" title="Click to change logo">
                            <img src={k2Logo} alt="Device" className="sync-logo-img" />
                        </div>
                    </div>
                    <div className="sync-column-content">
                        <div className="sync-content-title">
                            DEVICES
                            <button className="sync-add-btn-small" onClick={() => setIsAddDeviceOpen(true)} title="Add Device">+</button>
                        </div>
                        <div className="sync-item-list">
                            {devices.map(d => {
                                const isLinked = selectedFolder?.linkedDevices.includes(d.id);
                                return (
                                    <div 
                                        key={d.id} 
                                        className={`sync-list-item ${isLinked ? 'highlighted' : ''}`}
                                        onClick={() => toggleDeviceSelection(d.id)}
                                    >
                                        <div className="sync-device-select-v2">
                                            {isEditing && (
                                                <div className={`sync-k2-checkbox ${isLinked ? 'checked' : ''}`}></div>
                                            )}
                                            <div className="sync-device-info-v2">
                                                <div className={`sync-device-cable-icon ${d.status}`}></div>
                                                <span className="sync-item-name">{d.name}</span>
                                            </div>
                                        </div>
                                        <button 
                                            className="sync-remove-item-btn" 
                                            onClick={(e) => { e.stopPropagation(); handleRemoveDevice(d.id); }}
                                            title="Remove Device"
                                        >
                                            ✕
                                        </button>
                                    </div>
                                );
                            })}
                        </div>
                    </div>
                </div>
            </div>

            {/* Folder Info Panel */}
            {selectedFolder && (
                <div className="sync-folder-info-overlay">
                    <div className="sync-info-header">
                        <span className="sync-info-title">{isEditing ? 'Editing Sync Details' : 'Folder Details'}</span>
                        <div className="sync-info-id-wrapper">
                            <button 
                                className="sync-id-delete-icon" 
                                onClick={() => handleRemoveFolder(selectedFolder.id)}
                                title="Remove Folder from Sync"
                            >
                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                    <polyline points="3 6 5 6 21 6"></polyline>
                                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                                    <line x1="10" y1="11" x2="10" y2="17"></line>
                                    <line x1="14" y1="11" x2="14" y2="17"></line>
                                </svg>
                            </button>
                            <span className="sync-info-id">ID: {selectedFolder.id}</span>
                        </div>
                    </div>
                    
                    <div className="sync-info-body-grid">
                        <div className="sync-info-field">
                            <label>LOCAL PATH</label>
                            <span>{selectedFolder.path}</span>
                        </div>
                        
                        <div className="sync-info-field">
                            <label>CHECK INTERVAL</label>
                            {isEditing ? (
                                <select 
                                    className="sync-select-v2"
                                    value={selectedFolder.syncInterval}
                                    onChange={(e) => updateFolderOption('syncInterval', parseInt(e.target.value))}
                                >
                                    <option value={1}>1 Minute</option>
                                    <option value={10}>10 Minutes</option>
                                    <option value={30}>30 Minutes</option>
                                    <option value={60}>1 Hour</option>
                                </select>
                            ) : (
                                <span>{selectedFolder.syncInterval < 60 ? `${selectedFolder.syncInterval} mins` : '1 hour'}</span>
                            )}
                        </div>

                        <div className="sync-info-field">
                            <label>SYNC MODE</label>
                            {isEditing ? (
                                <div className="sync-mode-toggle">
                                    <button 
                                        className={`sync-mode-btn ${selectedFolder.syncMode === 'proactive' ? 'active' : ''}`}
                                        onClick={() => updateFolderOption('syncMode', 'proactive')}
                                    >
                                        PROACTIVE
                                    </button>
                                    <button 
                                        className={`sync-mode-btn ${selectedFolder.syncMode === 'passive' ? 'active' : ''}`}
                                        onClick={() => updateFolderOption('syncMode', 'passive')}
                                    >
                                        PASSIVE
                                    </button>
                                </div>
                            ) : (
                                <span className={selectedFolder.syncMode === 'proactive' ? 'sync-text-blue' : 'sync-text-orange'}>
                                    {selectedFolder.syncMode === 'proactive' ? 'Proactive (Auto)' : 'Passive (Manual)'}
                                </span>
                            )}
                        </div>

                        <div className="sync-info-field">
                            <label>SYNC TARGETS</label>
                            <span>{selectedFolder.linkedDevices.length} Devices</span>
                        </div>
                    </div>

                    <div className="sync-modal-footer">
                        {isEditing ? (
                            <button className="sync-k2-btn primary" onClick={() => setIsEditing(false)}>Save Changes</button>
                        ) : (
                            <>
                                <button className="sync-k2-btn">{selectedFolder.syncEnabled ? 'Pause' : 'Resume'}</button>
                                <button className="sync-k2-btn primary">Sync Now</button>
                                <button className="sync-k2-btn" onClick={() => setIsEditing(true)}>Edit Sync</button>
                            </>
                        )}
                    </div>
                </div>
            )}

            {/* Add Device Modal */}
            {isAddDeviceOpen && (
                <div className="sync-modal-overlay">
                    <div className="sync-k2-modal">
                        <h3>Register New Device</h3>
                        <div className="sync-modal-body">
                            <div className="sync-input-group">
                                <label>Node ID</label>
                                <input 
                                    type="text" 
                                    placeholder="Enter peer node-id (hex)..." 
                                    value={newNodeId}
                                    onChange={e => setNewNodeId(e.target.value)}
                                />
                            </div>
                            <div className="sync-input-group">
                                <label>Device Name</label>
                                <input 
                                    type="text" 
                                    placeholder="e.g. My Phone, Office PC..." 
                                    value={newDeviceName}
                                    onChange={e => setNewDeviceName(e.target.value)}
                                />
                            </div>
                        </div>
                        <div className="sync-modal-footer">
                            <button className="sync-k2-btn" onClick={() => setIsAddDeviceOpen(false)}>Cancel</button>
                            <button className="sync-k2-btn primary" onClick={handleAddDevice}>Register Device</button>
                        </div>
                    </div>
                </div>
            )}
            {/* Custom Confirmation Modal */}
            {confirmDialog.isOpen && (
                <div className="sync-modal-overlay">
                    <div className="sync-k2-modal" style={{ width: '400px' }}>
                        <h3 style={{ color: confirmDialog.actionType === 'danger' ? '#ff4444' : '#ffffff' }}>
                            {confirmDialog.title}
                        </h3>
                        <div className="sync-modal-body">
                            <p style={{ color: '#ccc', lineHeight: '1.6', fontSize: '14px' }}>
                                {confirmDialog.message}
                            </p>
                        </div>
                        <div className="sync-modal-footer">
                            <button 
                                className="sync-k2-btn" 
                                onClick={() => setConfirmDialog(prev => ({ ...prev, isOpen: false }))}
                            >
                                CANCEL
                            </button>
                            <button 
                                className={`sync-k2-btn ${confirmDialog.actionType === 'danger' ? 'danger' : 'primary'}`}
                                onClick={confirmDialog.onConfirm}
                            >
                                {confirmDialog.actionType === 'danger' ? 'CONFIRM REMOVE' : 'OK'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
