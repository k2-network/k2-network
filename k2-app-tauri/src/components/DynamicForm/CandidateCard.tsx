/**
 * CandidateCard Component
 * 
 * Displays a single candidate/match in the discovery results
 * Similar style to the Rent Order cards in the reference image
 */
import React from 'react';
import type { Candidate } from './types';
import './CandidateCard.css';

// Generate consistent color based on name
const getAvatarColor = (name: string): string => {
    const colors = [
        '#F15CDD', // Pink
        '#47E069', // Green
        '#4DA6FF', // Blue
        '#FFB84D', // Orange
        '#FF6B6B', // Red
        '#9B59B6', // Purple
        '#1ABC9C', // Teal
        '#F39C12', // Yellow
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

// Get action label (danh từ)
const getActionLabel = (action: string): string => {
    switch (action.toLowerCase()) {
        case 'buy':
            return 'Buyer';
        case 'sell':
            return 'Seller';
        case 'exchange':
            return 'Exchanger';
        default:
            return action;
    }
};

// Get status color and icon
const getStatusStyle = (status: Candidate['status']): { color: string; bgColor: string; icon: string } => {
    switch (status) {
        case 'active':
            return { color: '#47E069', bgColor: 'rgba(71, 224, 105, 0.15)', icon: '⚡' };
        case 'completed':
            return { color: '#4DA6FF', bgColor: 'rgba(77, 166, 255, 0.15)', icon: '✓' };
        case 'offline':
        default:
            return { color: '#858585', bgColor: 'rgba(133, 133, 133, 0.15)', icon: '○' };
    }
};

interface CandidateCardProps {
    candidate: Candidate;
    rank: number;
    onClick?: (candidate: Candidate) => void;
    selected?: boolean;
}

export const CandidateCard: React.FC<CandidateCardProps> = ({
    candidate,
    rank,
    onClick,
    selected = false
}) => {
    const avatarColor = getAvatarColor(candidate.name);
    const initials = getInitials(candidate.name);
    const actionLabel = getActionLabel(candidate.action);
    const statusStyle = getStatusStyle(candidate.status);

    // Format price range
    const formatPrice = (min: number, max: number, currency: string) => {
        const formatter = new Intl.NumberFormat('en-US', {
            style: 'currency',
            currency: currency,
            minimumFractionDigits: 0,
            maximumFractionDigits: 0
        });

        if (min === max) {
            return formatter.format(min);
        }
        return `${formatter.format(min)} - ${formatter.format(max)}`;
    };

    return (
        <div
            className={`candidate-card ${selected ? 'selected' : ''}`}
            onClick={() => onClick?.(candidate)}
        >
            {/* Header: Rank + Status */}
            <div className="candidate-header">
                <span className="candidate-rank">#{rank}</span>
                <span
                    className="candidate-status"
                    style={{
                        color: statusStyle.color,
                        backgroundColor: statusStyle.bgColor
                    }}
                >
                    <span className="status-icon">{statusStyle.icon}</span>
                    {candidate.status.charAt(0).toUpperCase() + candidate.status.slice(1)}
                </span>
            </div>

            {/* Title */}
            <h4 className="candidate-title">{candidate.title}</h4>

            {/* Price Range */}
            <div className="candidate-price">
                {formatPrice(
                    candidate.priceRange.min,
                    candidate.priceRange.max,
                    candidate.priceRange.currency
                )}
            </div>

            {/* Match Score Bar */}
            <div className="candidate-score-bar">
                <div
                    className="score-fill"
                    style={{ width: `${candidate.matchScore * 100}%` }}
                />
            </div>

            {/* Footer: Avatar + Name + Action */}
            <div className="candidate-footer">
                <div
                    className="candidate-avatar"
                    style={{ backgroundColor: avatarColor }}
                >
                    {initials}
                </div>
                <div className="candidate-info">
                    <span className="candidate-name">{candidate.name}</span>
                    <span className="candidate-action">{actionLabel}</span>
                </div>
                <button
                    className="candidate-action-btn"
                    onClick={(e) => {
                        e.stopPropagation();
                        onClick?.(candidate);
                    }}
                >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z" />
                    </svg>
                </button>
            </div>

            {/* Location badge (if available) */}
            {candidate.location && (
                <div className="candidate-location">
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M12 2C8.13 2 5 5.13 5 9c0 5.25 7 13 7 13s7-7.75 7-13c0-3.87-3.13-7-7-7zm0 9.5c-1.38 0-2.5-1.12-2.5-2.5s1.12-2.5 2.5-2.5 2.5 1.12 2.5 2.5-1.12 2.5-2.5 2.5z" />
                    </svg>
                    {candidate.location}
                </div>
            )}
        </div>
    );
};

export default CandidateCard;
