/**
 * NegotiationDashboard Component
 *
 * Real-time AI negotiation dashboard showing:
 * - Top 10 candidates ranking
 * - AI negotiation progress
 * - Live status updates
 * - Final results with copy nodeId feature
 */
import React, { useState, useEffect, useCallback } from 'react';
import { pingContact, sendInterest as apiSendInterest } from '../../api';
import type { Candidate, DynamicFormFields } from './types';
import './NegotiationDashboard.css';

// Generate consistent color based on name
const getAvatarColor = (name: string): string => {
    const colors = [
        '#F15CDD', '#47E069', '#4DA6FF', '#FFB84D',
        '#FF6B6B', '#9B59B6', '#1ABC9C', '#F39C12',
    ];
    let hash = 0;
    for (let i = 0; i < name.length; i++) {
        hash = name.charCodeAt(i) + ((hash << 5) - hash);
    }
    return colors[Math.abs(hash) % colors.length];
};

// Get initials from name
const getInitials = (name: string): string => {
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) {
        return (parts[0][0] + parts[1][0]).toUpperCase();
    }
    return name.substring(0, 2).toUpperCase();
};

interface NegotiationState {
    status: 'pending' | 'negotiating' | 'completed' | 'failed';
    score: number;
    aiNotes: string;
    rounds: number;
}

interface NegotiationDashboardProps {
    candidates: Candidate[];
    formData: DynamicFormFields | null;
    onComplete?: (results: Candidate[]) => void;
    onBack?: () => void;
    /** Gọi khi user confirm deal xong — dừng tìm kiếm và kết thúc */
    onDealDone?: () => void;
}

export const NegotiationDashboard: React.FC<NegotiationDashboardProps> = ({
    candidates,
    formData,
    onComplete,
    onBack,
    onDealDone,
}) => {
    const [negotiationStates, setNegotiationStates] = useState<Map<string, NegotiationState>>(new Map());
    const [currentNegotiating, setCurrentNegotiating] = useState<string | null>(null);
    const [isRunning, setIsRunning] = useState(false);
    const [completedCount, setCompletedCount] = useState(0);
    const [copiedNodeId, setCopiedNodeId] = useState<string | null>(null);
    const [dealDoneConfirmed, setDealDoneConfirmed] = useState(false);
    const [confirmingDeal, setConfirmingDeal] = useState(false);

    // Initialize negotiation states
    useEffect(() => {
        const states = new Map<string, NegotiationState>();
        candidates.forEach(c => {
            states.set(c.nodeId, {
                status: 'pending',
                score: 0,
                aiNotes: '',
                rounds: 0
            });
        });
        setNegotiationStates(states);
    }, [candidates]);

    // Real P2P negotiation for a single candidate
    const negotiateWithCandidate = useCallback(async (candidate: Candidate): Promise<NegotiationState> => {
        const totalRounds = 3;
        let currentScore = candidate.matchScore * 50;

        // Round 1: Check if peer is reachable (ping)
        setNegotiationStates(prev => {
            const newStates = new Map(prev);
            newStates.set(candidate.nodeId, {
                status: 'negotiating',
                score: Math.round(currentScore),
                aiNotes: `Round 1/${totalRounds}: Pinging peer...`,
                rounds: 1
            });
            return newStates;
        });

        let isOnline = false;
        try {
            isOnline = await pingContact(candidate.nodeId);
        } catch {
            isOnline = false;
        }
        const onlineBonus = isOnline ? 25 : 0;
        currentScore = Math.min(100, currentScore + onlineBonus);
        await new Promise(resolve => setTimeout(resolve, 800));

        // Round 2: Price match evaluation
        setNegotiationStates(prev => {
            const newStates = new Map(prev);
            newStates.set(candidate.nodeId, {
                status: 'negotiating',
                score: Math.round(currentScore),
                aiNotes: `Round 2/${totalRounds}: Evaluating price match...`,
                rounds: 2
            });
            return newStates;
        });

        const priceMatch = formData?.priceRange
            ? 1 - Math.abs(
                (candidate.priceRange.min + candidate.priceRange.max) / 2 -
                (formData.priceRange.min + formData.priceRange.max) / 2
            ) / Math.max(formData.priceRange.max, 1)
            : 0.5;
        const locationBonus = formData?.location === candidate.location ? 10 : 0;
        currentScore = Math.min(100, currentScore + priceMatch * 15 + locationBonus);
        await new Promise(resolve => setTimeout(resolve, 1000));

        // Round 3: Send interest if score is high enough and peer is online
        setNegotiationStates(prev => {
            const newStates = new Map(prev);
            newStates.set(candidate.nodeId, {
                status: 'negotiating',
                score: Math.round(currentScore),
                aiNotes: `Round 3/${totalRounds}: Sending interest...`,
                rounds: 3
            });
            return newStates;
        });

        if (isOnline && currentScore >= 60 && formData) {
            try {
                await apiSendInterest(
                    candidate.topic || formData.topic || 'Goods',
                    candidate.nodeId,
                    {
                        title: formData.title,
                        description: formData.description,
                        priceRange: formData.priceRange,
                        topic: formData.topic,
                        action: formData.action,
                    }
                );
                currentScore = Math.min(100, currentScore + 5);
            } catch (e) {
                console.warn('[Negotiation] send_interest failed:', e);
            }
        }
        await new Promise(resolve => setTimeout(resolve, 800));

        const notes = generateAINotes(candidate, currentScore, formData, isOnline);

        return {
            status: 'completed',
            score: Math.round(currentScore),
            aiNotes: notes,
            rounds: totalRounds
        };
    }, [formData]);

    // Generate AI analysis notes
    const generateAINotes = (candidate: Candidate, score: number, formData: DynamicFormFields | null, isOnline?: boolean): string => {
        const notes: string[] = [];

        if (formData?.priceRange) {
            const candidateMid = (candidate.priceRange.min + candidate.priceRange.max) / 2;
            const userMid = (formData.priceRange.min + formData.priceRange.max) / 2;

            if (candidateMid <= userMid * 1.1) {
                notes.push('Giá cả hợp lý');
            } else if (candidateMid <= userMid * 1.3) {
                notes.push('Giá hơi cao, có thể thương lượng');
            } else {
                notes.push('Giá cao hơn ngân sách');
            }
        }

        if (formData?.location && candidate.location) {
            if (formData.location.toLowerCase() === candidate.location.toLowerCase()) {
                notes.push('Cùng khu vực');
            } else {
                notes.push(`Khu vực: ${candidate.location}`);
            }
        }

        if (isOnline === true) {
            notes.push('Online (đã xác nhận)');
        } else if (isOnline === false) {
            notes.push('Không phản hồi ping');
        } else if (candidate.status === 'active') {
            notes.push('Đang hoạt động');
        }

        if (score >= 80) {
            notes.unshift('Rất phù hợp!');
        } else if (score >= 60) {
            notes.unshift('Phù hợp');
        } else {
            notes.unshift('Cần cân nhắc');
        }

        return notes.join(' • ');
    };

    // Start negotiation process
    const startNegotiation = useCallback(async () => {
        setIsRunning(true);
        setCompletedCount(0);

        const results: Map<string, NegotiationState> = new Map();

        for (const candidate of candidates) {
            setCurrentNegotiating(candidate.nodeId);

            try {
                const result = await negotiateWithCandidate(candidate);
                results.set(candidate.nodeId, result);
                setNegotiationStates(prev => {
                    const newStates = new Map(prev);
                    newStates.set(candidate.nodeId, result);
                    return newStates;
                });
            } catch (error) {
                const failedResult: NegotiationState = {
                    status: 'failed',
                    score: 0,
                    aiNotes: 'Không thể kết nối',
                    rounds: 0
                };
                results.set(candidate.nodeId, failedResult);
                setNegotiationStates(prev => {
                    const newStates = new Map(prev);
                    newStates.set(candidate.nodeId, failedResult);
                    return newStates;
                });
            }

            setCompletedCount(prev => prev + 1);
        }

        setCurrentNegotiating(null);
        setIsRunning(false);

        const rankedResults = [...candidates]
            .map(c => {
                const state = results.get(c.nodeId);
                return {
                    ...c,
                    negotiationScore: state?.score || 0,
                    aiNotes: state?.aiNotes || ''
                };
            })
            .sort((a, b) => (b.negotiationScore || 0) - (a.negotiationScore || 0));

        console.log('🎯 [NegotiationDashboard] Dispatching negotiationComplete:', rankedResults);

        window.dispatchEvent(new CustomEvent('k2:negotiationComplete', {
            detail: { candidates: rankedResults, formData }
        }));

        onComplete?.(rankedResults);
    }, [candidates, negotiateWithCandidate, formData, onComplete]);

    // Get candidates sorted by negotiation score
    const getRankedCandidates = useCallback((): Candidate[] => {
        return [...candidates]
            .map(c => ({
                ...c,
                negotiationScore: negotiationStates.get(c.nodeId)?.score || 0,
                aiNotes: negotiationStates.get(c.nodeId)?.aiNotes || ''
            }))
            .sort((a, b) => (b.negotiationScore || 0) - (a.negotiationScore || 0));
    }, [candidates, negotiationStates]);

    // Copy nodeId to clipboard
    const copyNodeId = (nodeId: string) => {
        navigator.clipboard.writeText(nodeId);
        setCopiedNodeId(nodeId);
        setTimeout(() => setCopiedNodeId(null), 2000);
    };

    // Open chat with candidate
    const openChat = (candidate: Candidate) => {
        window.dispatchEvent(new CustomEvent('k2:openChat', {
            detail: {
                nodeId: candidate.nodeId,
                name: candidate.name,
                deal: {
                    title: candidate.title,
                    priceMin: candidate.priceRange.min,
                    priceMax: candidate.priceRange.max,
                    currency: candidate.priceRange.currency,
                }
            }
        }));
    };

    const rankedCandidates = getRankedCandidates();
    const progress = candidates.length > 0 ? (completedCount / candidates.length) * 100 : 0;

    return (
        <div className="negotiation-dashboard">
            {/* Header */}
            <div className="negotiation-header">
                <div className="header-info">
                    <h2>AI Negotiation Dashboard</h2>
                    <p>
                        {isRunning
                            ? `Đang đàm phán với ${candidates.length} ứng viên...`
                            : completedCount === candidates.length && completedCount > 0
                                ? `Hoàn thành! ${completedCount}/${candidates.length} đã xử lý`
                                : `${candidates.length} ứng viên sẵn sàng`
                        }
                    </p>
                </div>

                <div className="header-actions">
                    {!isRunning && completedCount === 0 && (
                        <button
                            className="start-btn"
                            onClick={startNegotiation}
                            disabled={candidates.length === 0}
                        >
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                                <path d="M8 5v14l11-7z" />
                            </svg>
                            Bắt đầu đàm phán
                        </button>
                    )}

                    {!dealDoneConfirmed && !confirmingDeal && (
                        <button
                            className="deal-done-btn"
                            onClick={() => setConfirmingDeal(true)}
                            title="Xác nhận đã giao dịch xong — dừng tìm kiếm"
                        >
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                                <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41L9 16.17z" />
                            </svg>
                            Đã deal xong
                        </button>
                    )}

                    {confirmingDeal && !dealDoneConfirmed && (
                        <div className="deal-confirm-inline">
                            <span className="deal-confirm-label">
                                Kết thúc giao dịch{formData?.topic ? ` "${formData.topic}"` : ''}?
                            </span>
                            <button
                                className="deal-confirm-yes"
                                onClick={() => {
                                    setDealDoneConfirmed(true);
                                    setConfirmingDeal(false);
                                    onDealDone?.();
                                }}
                            >
                                Xác nhận
                            </button>
                            <button
                                className="deal-confirm-no"
                                onClick={() => setConfirmingDeal(false)}
                            >
                                Hủy
                            </button>
                        </div>
                    )}

                    {dealDoneConfirmed && (
                        <span className="deal-done-badge">
                            ✅ Giao dịch hoàn tất
                        </span>
                    )}

                    {onBack && (
                        <button className="back-btn" onClick={onBack}>
                            ← Quay lại
                        </button>
                    )}
                </div>
            </div>

            {/* Progress Bar */}
            {isRunning && (
                <div className="negotiation-progress">
                    <div className="progress-bar">
                        <div
                            className="progress-fill"
                            style={{ width: `${progress}%` }}
                        />
                    </div>
                    <span className="progress-text">{completedCount}/{candidates.length}</span>
                </div>
            )}

            {/* Rankings Table */}
            <div className="rankings-container">
                <div className="rankings-header">
                    <span className="col-rank">Rank</span>
                    <span className="col-candidate">Candidate</span>
                    <span className="col-price">Price Range</span>
                    <span className="col-status">Status</span>
                    <span className="col-score">Score</span>
                    <span className="col-action">Action</span>
                </div>

                <div className="rankings-list">
                    {rankedCandidates.map((candidate, index) => {
                        const state = negotiationStates.get(candidate.nodeId);
                        const isCurrentlyNegotiating = currentNegotiating === candidate.nodeId;
                        const avatarColor = getAvatarColor(candidate.name);

                        return (
                            <div
                                key={candidate.nodeId}
                                className={`ranking-row ${isCurrentlyNegotiating ? 'negotiating' : ''} ${state?.status || ''}`}
                            >
                                {/* Rank */}
                                <div className="col-rank">
                                    <span className={`rank-badge rank-${index + 1}`}>
                                        #{index + 1}
                                    </span>
                                </div>

                                {/* Candidate Info */}
                                <div className="col-candidate">
                                    <div
                                        className="ranking-avatar"
                                        style={{ backgroundColor: avatarColor }}
                                    >
                                        {getInitials(candidate.name)}
                                    </div>
                                    <div className="ranking-details">
                                        <span className="ranking-name">{candidate.name}</span>
                                        <span className="ranking-title">{candidate.title}</span>
                                        {state?.aiNotes && (
                                            <span className="ranking-notes">{state.aiNotes}</span>
                                        )}
                                    </div>
                                </div>

                                {/* Price */}
                                <div className="col-price">
                                    ${candidate.priceRange.min.toLocaleString()} - ${candidate.priceRange.max.toLocaleString()}
                                </div>

                                {/* Status */}
                                <div className="col-status">
                                    <span className={`status-badge ${state?.status || 'pending'}`}>
                                        {isCurrentlyNegotiating ? (
                                            <>
                                                <span className="status-dot pulsing" />
                                                Negotiating...
                                            </>
                                        ) : state?.status === 'completed' ? (
                                            <>
                                                <span className="status-dot completed" />
                                                Completed
                                            </>
                                        ) : state?.status === 'failed' ? (
                                            <>
                                                <span className="status-dot failed" />
                                                Failed
                                            </>
                                        ) : (
                                            <>
                                                <span className="status-dot pending" />
                                                Pending
                                            </>
                                        )}
                                    </span>
                                </div>

                                {/* Score */}
                                <div className="col-score">
                                    {state?.status === 'completed' ? (
                                        <div className="score-display">
                                            <span className={`score-value ${state.score >= 80 ? 'high' : state.score >= 60 ? 'medium' : 'low'}`}>
                                                {state.score}
                                            </span>
                                            <span className="score-max">/100</span>
                                        </div>
                                    ) : isCurrentlyNegotiating ? (
                                        <div className="score-loading">
                                            <div className="loading-bar" />
                                        </div>
                                    ) : (
                                        <span className="score-pending">--</span>
                                    )}
                                </div>

                                {/* Action */}
                                <div className="col-action">
                                    {state?.status === 'completed' ? (
                                        <button
                                            className="chat-open-btn"
                                            onClick={() => openChat(candidate)}
                                            title="Open Chat"
                                        >
                                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                                                <path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H6l-2 2V4h16v12z" />
                                            </svg>
                                            Chat
                                        </button>
                                    ) : (
                                        <button
                                            className={`copy-btn ${copiedNodeId === candidate.nodeId ? 'copied' : ''}`}
                                            onClick={() => copyNodeId(candidate.nodeId)}
                                            title="Copy Node ID"
                                        >
                                            {copiedNodeId === candidate.nodeId ? (
                                                <>
                                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                                                        <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41L9 16.17z" />
                                                    </svg>
                                                    Copied!
                                                </>
                                            ) : (
                                                <>
                                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                                                        <path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z" />
                                                    </svg>
                                                    Copy ID
                                                </>
                                            )}
                                        </button>
                                    )}
                                </div>
                            </div>
                        );
                    })}
                </div>
            </div>

            {/* Empty State */}
            {candidates.length === 0 && (
                <div className="empty-state">
                    <svg width="64" height="64" viewBox="0 0 24 24" fill="currentColor" opacity="0.3">
                        <path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H6l-2 2V4h16v12z" />
                    </svg>
                    <p>Chưa có ứng viên nào</p>
                    <span>Vui lòng tìm kiếm trước khi bắt đầu đàm phán</span>
                </div>
            )}
        </div>
    );
};

export default NegotiationDashboard;
