/**
 * CandidateCard Component
 * 
 * Displays a single candidate/match in the discovery results
 * Rent Order style cards with price range visualization
 */
import React from 'react';
import { IoFlash, IoCheckmarkCircle, IoEllipseOutline, IoLocationSharp, IoChevronForward } from 'react-icons/io5';
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

const getActionLabel = (action: string): string => {
    switch (action.toLowerCase()) {
        case 'buy': return 'Buyer';
        case 'sell': return 'Seller';
        case 'exchange': return 'Exchanger';
        default: return action;
    }
};

const getStatusConfig = (status: Candidate['status']) => {
    switch (status) {
        case 'active':
            return { color: '#47E069', bgColor: 'transparent', label: 'Active', Icon: IoFlash };
        case 'completed':
            return { color: '#4DA6FF', bgColor: 'transparent', label: 'Completed', Icon: IoCheckmarkCircle };
        case 'offline':
        default:
            return { color: '#858585', bgColor: 'transparent', label: 'Offline', Icon: IoEllipseOutline };
    }
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
    userPriceRange,
}) => {
    const avatarColor = getAvatarColor(candidate.name);
    const initials = getInitials(candidate.name);
    const actionLabel = getActionLabel(candidate.action);
    const status = getStatusConfig(candidate.status);

    const formatPrice = (value: number, currency: string) => {
        return new Intl.NumberFormat('en-US', {
            style: 'currency', currency, minimumFractionDigits: 0, maximumFractionDigits: 0
        }).format(value);
    };

    // Price range visualization:
    // - Track (gray) = User's range (my range) 
    // - Fill (white) = Candidate's range (their range) overlaid on my range
    const candMin = candidate.priceRange.min;
    const candMax = candidate.priceRange.max;
    const currency = candidate.priceRange.currency;

    // User's range (my range) - this defines the track
    const userMin = userPriceRange?.min ?? 0;
    const userMax = userPriceRange?.max ?? 1000;
    const userSpan = userMax - userMin || 1;

    // Calculate candidate's position within user's range
    // Clamp to 0-100%
    const candLeftPct = Math.max(0, Math.min(100, ((candMin - userMin) / userSpan) * 100));
    const candRightPct = Math.max(0, Math.min(100, ((candMax - userMin) / userSpan) * 100));
    const candWidthPct = candRightPct - candLeftPct;

    return (
        <div
            className={`candidate-card ${selected ? 'selected' : ''} status-${candidate.status}`}
            onClick={() => onClick?.(candidate)}
        >
            {/* Header: #rank | status ... location */}
            <div className="candidate-header">
                <div className="candidate-header-left">
                    <span className="candidate-rank">#{rank}</span>
                    <span className="header-divider">|</span>
                    <span
                        className="candidate-status"
                        style={{ color: status.color, backgroundColor: status.bgColor }}
                    >
                        <status.Icon className="status-icon" />
                        {status.label}
                    </span>
                </div>
                {candidate.location && (
                    <div className="candidate-location">
                        <IoLocationSharp size={12} />
                        {candidate.location}
                    </div>
                )}
            </div>

            {/* Title */}
            <h4 className="candidate-title">{candidate.title}</h4>

            {/* Price Range Visualization - User's range as track, Candidate's range as white fill */}
            <div className="price-range-section">
                <div className="price-range-labels">
                    <span className="price-label-min">{formatPrice(userMin, currency)}</span>
                    <span className="price-label-max">{formatPrice(userMax, currency)}</span>
                </div>
                <div className="price-range-track">
                    {/* White fill bar = Candidate's range */}
                    <div
                        className="price-range-fill"
                        style={{ left: `${candLeftPct}%`, width: `${candWidthPct}%` }}
                        title={`${actionLabel}: ${formatPrice(candMin, currency)} - ${formatPrice(candMax, currency)}`}
                    />
                    {/* Two hollow circle endpoints at candidate's range */}
                    <div className="price-endpoint" style={{ left: `${candLeftPct}%` }} />
                    <div className="price-endpoint" style={{ left: `${candLeftPct + candWidthPct}%` }} />
                </div>
            </div>

            {/* Footer: Avatar + Name + Action */}
            <div className="candidate-footer">
                <div className="candidate-avatar" style={{ backgroundColor: avatarColor }}>
                    {initials}
                </div>
                <div className="candidate-info">
                    <span className="candidate-name">{candidate.name}</span>
                    <span className="candidate-action">{actionLabel}</span>
                </div>
                <button
                    className="candidate-action-btn"
                    onClick={(e) => { e.stopPropagation(); onClick?.(candidate); }}
                >
                    <IoChevronForward size={20} />
                </button>
            </div>
        </div>
    );
};

export default CandidateCard;
