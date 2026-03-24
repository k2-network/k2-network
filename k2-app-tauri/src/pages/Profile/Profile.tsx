import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IoCopy, IoCheckmark, IoSparkles, IoTrophy, IoFlash } from "react-icons/io5";
import "./Profile.css";

// Pentagon/Radar chart component
const StatsRadar = ({ stats }: { stats: { label: string; value: number }[] }) => {
    const size = 180;
    const center = size / 2;
    const radius = 45;
    const points = stats.length;

    const getPoint = (index: number, value: number) => {
        const angle = (Math.PI * 2 * index) / points - Math.PI / 2;
        const r = (value / 100) * radius;
        return {
            x: center + r * Math.cos(angle),
            y: center + r * Math.sin(angle)
        };
    };

    const getLabelPoint = (index: number) => {
        const angle = (Math.PI * 2 * index) / points - Math.PI / 2;
        const r = radius + 20;
        return {
            x: center + r * Math.cos(angle),
            y: center + r * Math.sin(angle)
        };
    };

    const gridLevels = [25, 50, 75, 100];

    return (
        <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
            {gridLevels.map((level) => (
                <polygon
                    key={level}
                    points={stats.map((_, i) => {
                        const p = getPoint(i, level);
                        return `${p.x},${p.y}`;
                    }).join(' ')}
                    fill="none"
                    stroke="#222"
                    strokeWidth="1"
                />
            ))}
            {stats.map((_, i) => {
                const p = getPoint(i, 100);
                return (
                    <line key={i} x1={center} y1={center} x2={p.x} y2={p.y} stroke="#222" strokeWidth="1" />
                );
            })}
            <polygon
                points={stats.map((s, i) => {
                    const p = getPoint(i, s.value);
                    return `${p.x},${p.y}`;
                }).join(' ')}
                fill="rgba(77, 166, 255, 0.15)"
                stroke="#4DA6FF"
                strokeWidth="1.5"
            />
            {stats.map((s, i) => {
                const p = getPoint(i, s.value);
                return <circle key={i} cx={p.x} cy={p.y} r="2" fill="#4DA6FF" />;
            })}
            {stats.map((s, i) => {
                const p = getLabelPoint(i);
                return (
                    <text key={i} x={p.x} y={p.y} textAnchor="middle" dominantBaseline="middle" fill="#d3d3d3" fontSize="8">
                        {s.label}
                    </text>
                );
            })}
        </svg>
    );
};

// Trade card component matching reference
// const TradeCard = ({ symbol, date, change, status }: { symbol: string; date: string; change: number; status: string }) => {
//     // Mock mini chart bars
//     const bars = Array.from({ length: 30 }, () => Math.random() * 40 + 10);

//     return (
//         <div className="trade-card">
//             <div className="trade-header">
//                 <div className="trade-symbol-info">
//                     <div className="trade-icon">{symbol.charAt(0)}</div>
//                     <div>
//                         <div className="trade-symbol">{symbol}</div>
//                         <div className="trade-date">{date}</div>
//                     </div>
//                 </div>
//                 <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
//                     <span className="trade-badge">{status}</span>
//                     <span className={`trade-change ${change >= 0 ? 'positive' : 'negative'}`}>
//                         {change >= 0 ? '↑' : '↓'}{Math.abs(change)}%
//                     </span>
//                 </div>
//             </div>

//             <div className="trade-meta">
//                 <span>Risk: Degen</span>
//                 <span>Risk/Reward: 0.63</span>
//             </div>

//             <div className="trade-chart">
//                 {bars.map((h, i) => (
//                     <div key={i} className="chart-bar" style={{ height: `${h}%` }} />
//                 ))}
//             </div>

//             <div className="trade-status-row">
//                 <span className="trade-status">POSITION TAKEN</span>
//             </div>

//             <div className="trade-footer">
//                 <div className="trade-actions">
//                     <button className="trade-action"><IoThumbsUp /> 1</button>
//                     <button className="trade-action"><IoChatbubble /></button>
//                     <button className="trade-action"><IoBookmark /></button>
//                 </div>
//                 <div className="trade-menu">
//                     <button className="trade-menu-btn"><IoShareSocial /></button>
//                     <button className="trade-menu-btn"><IoEllipsisHorizontal /></button>
//                 </div>
//             </div>
//         </div>
//     );
// };

export function ProfilePage() {
    const [nodeId, setNodeId] = useState<string>("");
    const [copied, setCopied] = useState(false);
    const [username, setUsername] = useState(() => localStorage.getItem('k2_username') || 'Anonymous');
    const [isEditingName, setIsEditingName] = useState(false);

    useEffect(() => {
        const fetchNodeId = async () => {
            try {
                const id = await invoke<string>('get_my_node_id');
                setNodeId(id);
            } catch (err) {
                console.error('Failed to get node id:', err);
            }
        };
        fetchNodeId();
    }, []);

    const copyNodeId = () => {
        if (!nodeId) return;
        navigator.clipboard.writeText(nodeId);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const handleNameSave = (newName: string) => {
        const name = newName.trim() || 'Anonymous';
        setUsername(name);
        localStorage.setItem('k2_username', name);
        setIsEditingName(false);
    };

    const radarStats = [
        { label: "TRADES", value: 75 },
        { label: "REWARD", value: 68 },
        { label: "DRAWDOWN", value: 45 },
        { label: "RATE", value: 82 },
        { label: "RETURN", value: 55 }
    ];

    // const trades = [
    //     { symbol: "MSFT", date: "7/17/25 22c @1.97", change: 10, status: "OPEN" },
    //     { symbol: "AAPL", date: "7/13/25 19c @1.91", change: -10, status: "OPEN" },
    //     { symbol: "NVDA", date: "6/30/25 23c @1.84", change: 10, status: "OPEN" }
    // ];

    const shortNodeId = nodeId ? `${nodeId.slice(0, 12)}...${nodeId.slice(-8)}` : "Loading...";

    return (
        <div className="profile-page">
            {/* Left Panel */}
            <div className="profile-left">
                {/* Header */}
                <div className="profile-header">
                    <div className="profile-avatar">
                        <span>{username.charAt(0).toUpperCase()}</span>
                    </div>
                    <div className="profile-name-section">
                        {isEditingName ? (
                            <input
                                className="profile-name-input"
                                defaultValue={username}
                                autoFocus
                                onBlur={(e) => handleNameSave(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === 'Enter') handleNameSave(e.currentTarget.value);
                                    if (e.key === 'Escape') setIsEditingName(false);
                                }}
                            />
                        ) : (
                            <h2 className="profile-name" onClick={() => setIsEditingName(true)} title="Click to edit">
                                {username} <IoSparkles className="name-icon" />
                            </h2>
                        )}
                        <div className="profile-subscribers">59 subscribers</div>
                    </div>
                </div>

                {/* Badges */}
                <div className="profile-badges">
                    <span className="badge badge-primary"><IoTrophy /> TOP 1</span>
                    <span className="badge badge-secondary"><IoFlash /> CONTRIBUTOR </span>
                </div>

                {/* Node ID */}
                <div className="node-id-section">
                    <div className="node-id-label">Your Node ID</div>
                    <div className="node-id-box">
                        <code className="node-id-text">{shortNodeId}</code>
                        <button className={`copy-btn ${copied ? 'copied' : ''}`} onClick={copyNodeId} title={copied ? 'Copied!' : 'Copy'}>
                            {copied ? <IoCheckmark /> : <IoCopy />}
                        </button>
                    </div>
                    <div className="node-id-hint">Share this ID with others to connect</div>
                </div>

                {/* Tabs */}
                <div className="profile-tabs">
                    <button className="profile-tab active">My Trades</button>
                    <button className="profile-tab">Bookmarked</button>
                </div>

                {/* Filter */}
                <div className="trades-filter">
                    <span className="filter-item active">Recents</span>
                    <span className="filter-item">Popular</span>
                    <span className="filter-item">Profitable</span>
                </div>

                {/* Trades */}
                {/* {trades.map((trade, i) => (
                    <TradeCard key={i} symbol={trade.symbol} date={trade.date} change={trade.change} status={trade.status} />
                ))} */}
            </div>

            {/* Right Panel - Stats */}
            <div className="profile-right">
                <div className="stats-title">TOTAL TRADES</div>

                <div className="radar-section">
                    <StatsRadar stats={radarStats} />
                </div>

                <div className="stats-grid">
                    <div className="stat-card">
                        <div className="stat-label">AVG. WIN</div>
                        <div className="stat-value highlight">80.21<span className="stat-suffix">%</span></div>
                    </div>
                    <div className="stat-card">
                        <div className="stat-label">AVG. LOSS</div>
                        <div className="stat-value">15.94<span className="stat-suffix">%</span></div>
                    </div>
                </div>

                <div className="stats-single">
                    <div className="stat-card">
                        <div className="stat-label">WIN RATE</div>
                        <div className="stat-value highlight">68.75<span className="stat-suffix">%</span></div>
                    </div>
                    <div className="progress-bar"><div className="progress-fill" style={{ width: '68%' }} /></div>
                </div>

                <div className="stats-grid">
                    <div className="stat-card">
                        <div className="stat-label">TOTAL</div>
                        <div className="stat-value">80</div>
                    </div>
                    <div className="stat-card">
                        <div className="stat-label">SUM GAIN</div>
                        <div className="stat-value highlight">2,451<span className="stat-suffix">%</span></div>
                    </div>
                </div>
            </div>
        </div>
    );
}
