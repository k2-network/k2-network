/**
 * CandidateList Component
 *
 * Displays a list of matched candidates with:
 * - Search bar
 * - Start Negotiation button
 * - Table ranking layout (matching NegotiationDashboard design)
 * - Sorting/ranking based on price, location, and other criteria
 */
import React, { useState, useMemo } from 'react';
import type { Candidate, DynamicFormFields } from './types';
import './CandidateList.css';

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

interface CandidateListProps {
    candidates: Candidate[];
    formData: DynamicFormFields | null;
    onStartNegotiation: (selectedCandidates: Candidate[]) => void;
    onCandidateClick?: (candidate: Candidate) => void;
    maxDisplay?: number;
}

// Sorting/ranking algorithm
const rankCandidates = (
    candidates: Candidate[],
    formData: DynamicFormFields | null
): Candidate[] => {
    if (!formData) return candidates;

    return [...candidates].sort((a, b) => {
        const userPriceMin = formData.priceRange?.min || 0;
        const userPriceMax = formData.priceRange?.max || 999999;
        const userPriceMid = (userPriceMin + userPriceMax) / 2;

        const aPriceMid = (a.priceRange.min + a.priceRange.max) / 2;
        const bPriceMid = (b.priceRange.min + b.priceRange.max) / 2;

        const aPriceDiff = Math.abs(aPriceMid - userPriceMid);
        const bPriceDiff = Math.abs(bPriceMid - userPriceMid);

        const aLocationMatch = formData.location && a.location?.toLowerCase() === formData.location.toLowerCase() ? 0 : 1;
        const bLocationMatch = formData.location && b.location?.toLowerCase() === formData.location.toLowerCase() ? 0 : 1;

        const aMatchScore = a.matchScore || 0;
        const bMatchScore = b.matchScore || 0;

        const statusOrder = { active: 0, offline: 1, completed: 2 };
        const aStatusScore = statusOrder[a.status] || 1;
        const bStatusScore = statusOrder[b.status] || 1;

        const aScore = (aPriceDiff / 1000) + (aLocationMatch * 100) - (aMatchScore * 50) + (aStatusScore * 10);
        const bScore = (bPriceDiff / 1000) + (bLocationMatch * 100) - (bMatchScore * 50) + (bStatusScore * 10);

        return aScore - bScore;
    });
};

export const CandidateList: React.FC<CandidateListProps> = ({
    candidates,
    formData,
    onStartNegotiation,
    onCandidateClick,
    maxDisplay = 10
}) => {
    const [searchQuery, setSearchQuery] = useState('');
    const [selectedCandidates, setSelectedCandidates] = useState<Set<string>>(new Set());

    const rankedCandidates = useMemo(() => {
        const filtered = candidates.filter(c => {
            if (!searchQuery) return true;
            const query = searchQuery.toLowerCase();
            return (
                c.name.toLowerCase().includes(query) ||
                c.title.toLowerCase().includes(query) ||
                c.location?.toLowerCase().includes(query)
            );
        });
        return rankCandidates(filtered, formData).slice(0, maxDisplay);
    }, [candidates, formData, searchQuery, maxDisplay]);

    const toggleSelection = (candidate: Candidate) => {
        const newSelected = new Set(selectedCandidates);
        if (newSelected.has(candidate.nodeId)) {
            newSelected.delete(candidate.nodeId);
        } else {
            newSelected.add(candidate.nodeId);
        }
        setSelectedCandidates(newSelected);
        onCandidateClick?.(candidate);
    };

    const handleStartNegotiation = () => {
        const selected = rankedCandidates.filter(c =>
            selectedCandidates.size === 0 || selectedCandidates.has(c.nodeId)
        );
        onStartNegotiation(selected);
    };

    return (
        <div className="candidate-list">
            {/* Header with Search and Action */}
            <div className="candidate-list-header">
                <div className="search-container">
                    <svg className="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z" />
                    </svg>
                    <input
                        type="text"
                        placeholder="Search candidates..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="search-input"
                    />
                </div>

                <button
                    className="start-negotiation-btn"
                    onClick={handleStartNegotiation}
                    disabled={rankedCandidates.length === 0}
                >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M8 5v14l11-7z" />
                    </svg>
                    Start Negotiation
                    {selectedCandidates.size > 0 && (
                        <span className="selected-count">({selectedCandidates.size})</span>
                    )}
                </button>
            </div>

            {/* Results Info */}
            <div className="candidate-list-info">
                <span className="results-count">
                    {rankedCandidates.length} matches found
                </span>
                <span className="sort-info">
                    Sorted by: Price, Location, Match Score
                </span>
            </div>

            {/* Rankings Table */}
            <div className="rankings-container">
                <div className="rankings-header">
                    <span className="col-rank">Rank</span>
                    <span className="col-candidate">Candidate</span>
                    <span className="col-price">Price Range</span>
                    <span className="col-status">Status</span>
                    <span className="col-score">Match</span>
                </div>

                <div className="rankings-list">
                    {rankedCandidates.map((candidate, index) => {
                        const avatarColor = getAvatarColor(candidate.name);
                        const isSelected = selectedCandidates.has(candidate.nodeId);

                        return (
                            <div
                                key={candidate.nodeId}
                                className={`ranking-row ${isSelected ? 'selected' : ''} status-${candidate.status}`}
                                onClick={() => toggleSelection(candidate)}
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
                                        {candidate.status === 'active' ? 'Active' :
                                            candidate.status === 'completed' ? 'Completed' : 'Offline'}
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
                    })}
                </div>
            </div>

            {/* Empty State */}
            {rankedCandidates.length === 0 && (
                <div className="candidate-list-empty">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="currentColor" opacity="0.3">
                        <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z" />
                    </svg>
                    <p>No matches found</p>
                    <span>Try adjusting your search or wait for more candidates</span>
                </div>
            )}
        </div>
    );
};

export default CandidateList;
