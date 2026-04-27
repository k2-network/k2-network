import React, { useState, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import "./SyncPage.css";
import fragmentIcon from "../../assets/icons/fragment.svg";
import k2Logo from "../../assets/icons/k2-logo.svg";
import syncLogo from "../../assets/sync-logo.svg";
import folderIcon from "../../assets/icons/folder.svg";

import { invoke } from "@tauri-apps/api/core";

interface FolderInfo {
    id: string;
    name: string;
    path: string;
    syncInterval: number;
    syncMode: 'proactive' | 'passive';
    syncEnabled: boolean;
    linkedDevices: Record<string, string>; // NodeID -> Status
    status: 'Idle' | 'Pending' | 'Syncing' | { Error: string };
    lastScan?: string;
    isPending?: boolean;
    remoteSource?: string;
}

interface BackendDeviceInfo {
    config: {
        id: string;
        name: string;
        deviceType: string;
        nodeId: string;
    };
    status: string;
}

interface DeviceInfo {
    id: string;
    name: string;
    deviceType: string;
    nodeId: string;
    status: "online" | "offline";
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
    const [toastMessage, setToastMessage] = useState<string | null>(null);

    // Initial Load from iroh-docs via Tauri
    useEffect(() => {
        const loadAll = async () => {
            try {
                const [f, d, s] = await Promise.all([
                    invoke<FolderInfo[]>('get_sync_folders'),
                    invoke<BackendDeviceInfo[]>('get_sync_devices'),
                    invoke<SyncSettings>('get_sync_settings')
                ]);
                setFolders(f);
                setDevices(d.map(b => ({
                    id: b.config.id,
                    name: b.config.name,
                    deviceType: b.config.deviceType,
                    nodeId: b.config.nodeId,
                    status: b.status as 'online' | 'offline'
                })));
                if (s.localLogo) setLocalLogo(s.localLogo);
                console.log("[Sync] Loaded config and status from core");
            } catch (error) {
                console.error("[Sync] Failed to load initial state:", error);
            }
        };
        loadAll();

        // Start polling for status updates every 3 seconds
        const pollInterval = setInterval(async () => {
            try {
                const [updatedFolders, updatedDevices] = await Promise.all([
                    invoke<FolderInfo[]>('get_sync_folders'),
                    invoke<BackendDeviceInfo[]>('get_sync_devices')
                ]);
                setFolders(updatedFolders);
                setDevices(updatedDevices.map(b => ({
                    id: b.config.id,
                    name: b.config.name,
                    deviceType: b.config.deviceType,
                    nodeId: b.config.nodeId,
                    status: b.status as 'online' | 'offline'
                })));
            } catch (err) {
                console.error("[Sync] Polling error:", err);
            }
        }, 3000);

        return () => clearInterval(pollInterval);
    }, []);

    const showToast = (msg: string) => {
        setToastMessage(msg);
        setTimeout(() => setToastMessage(null), 3000);
    };

    // Helper to sync a single folder update to backend

    const syncFolderToBackend = async (folder: FolderInfo) => {
        try {
            // Clean up any "undefined" keys that might have leaked into the state
            const cleanedLinkedDevices = { ...folder.linkedDevices };
            if ("undefined" in cleanedLinkedDevices) delete cleanedLinkedDevices["undefined"];
            if (undefined in cleanedLinkedDevices) delete cleanedLinkedDevices[undefined as any];
            
            const cleanedFolder = { ...folder, linkedDevices: cleanedLinkedDevices };
            await invoke('add_sync_folder', { config: cleanedFolder });
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
    const [isTestingConnection, setIsTestingConnection] = useState(false);
    const [testResult, setTestResult] = useState<'success' | 'fail' | null>(null);

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

    const toggleDeviceSelection = async (nodeId: string) => {
        if (!selectedFolderId || !isEditing) return;
        
        const folder = folders.find(f => f.id === selectedFolderId);
        if (!folder) return;

        const newLinkedDevices = { ...folder.linkedDevices };
        if (newLinkedDevices[nodeId]) {
            delete newLinkedDevices[nodeId];
        } else {
            newLinkedDevices[nodeId] = "NotSent";
        }
            
        const updatedFolder = { ...folder, linkedDevices: newLinkedDevices };
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
            const selected = await open({
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
                    syncInterval: 60,
                    syncMode: 'passive',
                    linkedDevices: {},
                    status: 'Idle'
                };
                
                // Update UI Optimistically (but without the real ID yet)
                setFolders(prev => [...prev, newFolder]);
                setSelectedFolderId(newFolder.id);
                setIsEditing(true);
                
                try {
                    // Get real ID from Backend
                    const realId = await invoke<string>('add_sync_folder', { config: newFolder });
                    
                    // Immediately re-fetch all folders to get correct data
                    const updatedFolders = await invoke<FolderInfo[]>('get_sync_folders');
                    setFolders(updatedFolders);
                    setSelectedFolderId(realId); // Select the real folder
                } catch (error) {
                    console.error("[Sync] Error adding folder:", error);
                    // Revert optimistic update
                    setFolders(prev => prev.filter(f => f.id !== newFolder.id));
                    setSelectedFolderId(null);
                }
            }
        } catch (error) {
            console.error("Failed to open folder dialog:", error);
        }
    };

    const handleAcceptFolder = async (folderId: string) => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: "Select Local Path for Shared Folder"
            });

            if (selected && typeof selected === 'string') {
                await invoke("accept_sync_folder", { folderId, localPath: selected });
                // Re-fetch folders to update the UI from pending to active
                const updatedFolders: FolderInfo[] = await invoke("get_sync_folders");
                setFolders(updatedFolders);
                setSelectedFolderId(folderId);
            }
        } catch (error) {
            console.error("Failed to accept folder:", error);
            alert("Failed to accept folder: " + error);
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

    const handleRemoveFolder = async (id: string) => {
        setConfirmDialog({
            isOpen: true,
            title: "Remove Sync Folder",
            message: "Are you sure you want to stop syncing this folder? This will only remove the sync configuration, not your local files.",
            actionType: 'danger',
            onConfirm: async () => {
                try {
                    await invoke('remove_sync_folder', { id });
                    setSelectedFolderId(null);
                    setFolders(prev => prev.filter(f => f.id !== id));
                } catch (error) {
                    console.error("[Sync] Failed to remove folder:", error);
                }
                setConfirmDialog(prev => ({ ...prev, isOpen: false }));
            }
        });
    };

    const handleAcceptInvitation = async (folderId: string) => {
        // @ts-ignore - Tauri API
        const path = await open({
            directory: true,
            multiple: false,
            title: "Select Folder to Sync"
        });

        if (path && typeof path === 'string') {
            try {
                await invoke('accept_sync_folder', { folderId, localPath: path });
                // Refresh folder list
                const updatedFolders = await invoke<FolderInfo[]>('get_sync_folders');
                setFolders(updatedFolders);
                // Update selected folder
                const newSelected = updatedFolders.find(f => f.id === folderId);
                if (newSelected) setSelectedFolderId(newSelected.id);
            } catch (error) {
                console.error("[Sync] Failed to accept invitation:", error);
            }
        }
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
                    const updatedFolders = folders.map(f => {
                        const newLinked = { ...f.linkedDevices };
                        // Find and remove by nodeId if the id matches
                        const deviceToRemove = devices.find(d => d.id === id);
                        if (deviceToRemove) {
                            delete newLinked[deviceToRemove.nodeId];
                        }
                        return { ...f, linkedDevices: newLinked };
                    });

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
    const handleTestConnection = async () => {
        if (!newNodeId) return;
        setIsTestingConnection(true);
        setTestResult(null);
        try {
            const isOnline = await invoke<boolean>('test_sync_device', { nodeId: newNodeId });
            setTestResult(isOnline ? 'success' : 'fail');
        } catch (err) {
            console.error("Test connection failed:", err);
            setTestResult('fail');
        } finally {
            setIsTestingConnection(false);
        }
    };

    const handleSyncNow = async (id: string) => {
        try {
            console.log(`[Sync] Manually triggering sync for ${id}`);
            await invoke('sync_now', { id });
            showToast("Sync completed successfully!");
        } catch (err) {
            console.error("Failed to sync now:", err);
            alert("Sync failed: " + err);
        }
    };

    const getStatusText = (status: FolderInfo['status']) => {
        if (status === 'Idle') return 'Up to date';
        if (status === 'Syncing') return 'Syncing...';
        if (status === 'Pending') return 'Changes pending';
        if (typeof status === 'object' && 'Error' in status) return `Error: ${status.Error}`;
        return status;
    };

    return (
        <div className="sync-page-v2">
            {toastMessage && (
                <div className="sync-toast">
                    {toastMessage}
                </div>
            )}
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
                                    className={`sync-list-item ${selectedFolderId === f.id ? 'active' : ''} ${f.isPending ? 'pending' : ''}`}
                                    onClick={() => { 
                                        if (f.isPending) return; // Don't select pending folders yet
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
                                        <div className="sync-item-text-wrapper">
                                            <span className="sync-item-name">{f.name}</span>
                                            {f.isPending && <span className="sync-item-sub">Invite from Network</span>}
                                        </div>
                                    </div>
                                    {f.isPending ? (
                                        <button 
                                            className="sync-accept-badge" 
                                            onClick={(e) => { e.stopPropagation(); handleAcceptFolder(f.id); }}
                                        >
                                            Accept
                                        </button>
                                    ) : (
                                        <div className={`sync-item-status-dot ${f.syncEnabled ? 'syncing' : 'paused'}`}></div>
                                    )}
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
                                const isLinked = selectedFolder?.linkedDevices ? d.nodeId in selectedFolder.linkedDevices : false;
                                return (
                                    <div 
                                        key={d.id} 
                                        className={`sync-list-item ${isLinked ? 'highlighted' : ''}`}
                                        onClick={() => toggleDeviceSelection(d.nodeId)}
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

                        <div className="sync-info-field full-width">
                            <label>SYNC TARGETS</label>
                            <div className="sync-targets-list">
                                {devices.map(device => {
                                    const status = selectedFolder.linkedDevices[device.nodeId];
                                    const isLinked = !!status;
                                    
                                    return (
                                        <div 
                                            key={device.id} 
                                            className={`device-toggle-card ${isLinked ? 'active' : ''} ${isEditing ? 'editable' : 'readonly'}`}
                                            onClick={() => toggleDeviceSelection(device.nodeId)}
                                        >
                                            <div className="device-avatar">
                                                {device.name.charAt(0)}
                                            </div>
                                            <div className="device-info-mini">
                                                <span className="dev-name">{device.name}</span>
                                                <span className="dev-type">{device.deviceType}</span>
                                                {isLinked && <span className="dev-status-badge">{status}</span>}
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>

                        <div className="sync-info-field full-width">
                            <label>CURRENT STATUS</label>
                            <div className={`sync-status-badge ${typeof selectedFolder.status === 'string' ? selectedFolder.status.toLowerCase() : 'error'}`}>
                                {getStatusText(selectedFolder.status)}
                            </div>
                        </div>
                    </div>

                    <div className="sync-modal-footer">
                        {selectedFolder.isPending ? (
                            <button 
                                className="sync-k2-btn primary" 
                                onClick={() => handleAcceptInvitation(selectedFolder.id)}
                            >
                                Accept & Choose Folder
                            </button>
                        ) : isEditing ? (
                            <button 
                                className="sync-k2-btn primary" 
                                onClick={async () => {
                                    if (selectedFolder) await syncFolderToBackend(selectedFolder);
                                    setIsEditing(false);
                                }}
                            >
                                Save Changes
                            </button>
                        ) : (
                            <>
                                <button 
                                    className={`sync-k2-btn ${selectedFolder.syncEnabled ? '' : 'primary'}`}
                                    onClick={() => updateFolderOption('syncEnabled', !selectedFolder.syncEnabled)}
                                >
                                    {selectedFolder.syncEnabled ? 'Pause Sync' : 'Resume Sync'}
                                </button>
                                <button 
                                    className="sync-k2-btn primary" 
                                    onClick={() => handleSyncNow(selectedFolder.id)}
                                    disabled={selectedFolder.status === 'Syncing'}
                                >
                                    {selectedFolder.status === 'Syncing' ? 'Syncing...' : 'Sync Now'}
                                </button>
                                <button className="sync-k2-btn" onClick={() => setIsEditing(true)}>Edit Configuration</button>
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
                                <div className="sync-input-wrapper">
                                    <input 
                                        type="text" 
                                        placeholder="Enter peer node-id (hex)..." 
                                        value={newNodeId}
                                        onChange={e => { setNewNodeId(e.target.value); setTestResult(null); }}
                                    />
                                    <button 
                                        className={`sync-test-btn ${testResult || ''}`}
                                        onClick={handleTestConnection}
                                        disabled={isTestingConnection || !newNodeId}
                                    >
                                        {isTestingConnection ? 'Testing...' : 'Test'}
                                    </button>
                                </div>
                                {testResult && (
                                    <div className={`sync-item-sub ${testResult}`} style={{ marginTop: '6px', fontWeight: 'bold' }}>
                                        {testResult === 'success' ? '● Device is ONLINE' : '○ Device is OFFLINE or ID is invalid'}
                                    </div>
                                )}
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
                            <button className="sync-k2-btn" onClick={() => { setIsAddDeviceOpen(false); setTestResult(null); }}>Cancel</button>
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
