/**
 * CandidateCard Component
 *
 * Displays a single candidate as a table row — matching NegotiationDashboard design.
 */
import React from 'react';
import type { Candidate } from './types';
import './CandidateCard.css';

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

const getInitials = (name: string): string => {
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
    return name.substring(0, 2).toUpperCase();
};

interface CandidateCardProps {
    candidate: Candidate;
    rank: number;
    onClick?: (candidate: Candidate) => void;
    selected?: boolean;
    userPriceRange?: { min: number; max: number; currency: string };
}

export const CandidateCard: React.FC<CandidateCardProps> = ({
    candidate,
    rank,
    onClick,
    selected = false,
}) => {
    const avatarColor = getAvatarColor(candidate.name);

    const statusLabel =
        candidate.status === 'active' ? 'Active' :
        candidate.status === 'completed' ? 'Completed' : 'Offline';

    return (
        <div
            className={`candidate-card-row ${selected ? 'selected' : ''} status-${candidate.status}`}
            onClick={() => onClick?.(candidate)}
        >
            {/* Rank */}
            <div className="col-rank">
                <span className={`rank-badge rank-${rank}`}>#{rank}</span>
            </div>

            {/* Candidate Info */}
            <div className="col-candidate">
                <div className="ranking-avatar" style={{ backgroundColor: avatarColor }}>
                    {getInitials(candidate.name)}
                </div>
                <div className="ranking-details">
                    <span className="ranking-name">{candidate.name}</span>
                    <span className="ranking-title">{candidate.title}</span>
                    {candidate.location && (
                        <span className="ranking-location">{candidate.location}</span>
                    )}
                </div>
            </div>

            {/* Price */}
            <div className="col-price">
                ${candidate.priceRange.min.toLocaleString()} - ${candidate.priceRange.max.toLocaleString()}
            </div>

            {/* Status */}
            <div className="col-status">
                <span className={`status-badge status-${candidate.status}`}>
                    {statusLabel}
                </span>
            </div>

            {/* Match Score */}
            <div className="col-score">
                <div className="match-score-display">
                    <span className="match-score-value">
                        {Math.round((candidate.matchScore || 0) * 100)}
                    </span>
                    <span className="match-score-max">/100</span>
                </div>
            </div>
        </div>
    );
};

export default CandidateCard;
