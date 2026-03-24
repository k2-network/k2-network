import React, { useState, useEffect } from 'react';
import type { ActiveOffer } from './types';

interface FindMatchingViewProps {
    offers: ActiveOffer[];
    onStartNegotiation: (offerId: string) => void;
    onCancelOffer: (offerId: string) => void;
}

function formatTimeAgo(createdAt: number): string {
    const diffMs = Date.now() - createdAt;
    const diffSec = Math.floor(diffMs / 1000);
    if (diffSec < 60) return `${diffSec} giây trước`;
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return `${diffMin} phút trước`;
    const diffHr = Math.floor(diffMin / 60);
    return `${diffHr} giờ trước`;
}

const ACTION_LABELS: Record<string, string> = {
    buy: 'MUA',
    sell: 'BÁN',
    exchange: 'TRAO ĐỔI',
};

const ACTION_COLORS: Record<string, string> = {
    buy: '#4DA6FF',
    sell: '#F97316',
    exchange: '#A78BFA',
};

function OfferCard({
    offer,
    onStartNegotiation,
    onCancelOffer,
}: {
    offer: ActiveOffer;
    onStartNegotiation: (offerId: string) => void;
    onCancelOffer: (offerId: string) => void;
}) {
    const [timeAgo, setTimeAgo] = useState(() => formatTimeAgo(offer.createdAt));

    useEffect(() => {
        const interval = setInterval(() => {
            setTimeAgo(formatTimeAgo(offer.createdAt));
        }, 10000);
        return () => clearInterval(interval);
    }, [offer.createdAt]);

    const accentColor = ACTION_COLORS[offer.formData.action] || '#4DA6FF';
    const subtopic = offer.formData.selection && 'subtopic' in offer.formData.selection
        ? offer.formData.selection.subtopic
        : offer.formData.selection && 'category' in offer.formData.selection
            ? offer.formData.selection.category
            : null;

    return (
        <div className="find-matching-card" style={{ '--card-accent': accentColor } as React.CSSProperties}>
            <div className="fmc-header">
                <span
                    className="fmc-action-badge"
                    style={{ background: `${accentColor}22`, color: accentColor, border: `1px solid ${accentColor}55` }}
                >
                    {ACTION_LABELS[offer.formData.action] || offer.formData.action.toUpperCase()}
                </span>
                <span className="fmc-title">{offer.formData.title}</span>
            </div>

            <div className="fmc-meta">
                <span className="fmc-topic">{offer.formData.topic}</span>
                {subtopic && (
                    <>
                        <span className="fmc-sep">›</span>
                        <span className="fmc-subtopic">{subtopic}</span>
                    </>
                )}
                <span className="fmc-sep">•</span>
                <span className="fmc-time">{timeAgo}</span>
            </div>

            <div className="fmc-status">
                {offer.candidates.length > 0 ? (() => {
                    const sorted = [...offer.candidates].sort((a, b) => (b.matchScore ?? 0) - (a.matchScore ?? 0));
                    const best = sorted[0];
                    return (
                        <div className="fmc-found">
                            <span className="fmc-dot fmc-dot-active" />
                            <span className="fmc-found-text">
                                {offer.candidates.length} đối tác
                                {best?.matchScore ? ` • Best: ${Math.round(best.matchScore * 100)}%` : ''}
                            </span>
                        </div>
                    );
                })() : (
                    <div className="fmc-searching">
                        <span className="fmc-dot fmc-dot-pulse" />
                        <span className="fmc-searching-text">Đang tìm kiếm đối tác...</span>
                    </div>
                )}
            </div>

            <div className="fmc-actions">
                <button
                    className="fmc-btn-negotiate"
                    style={{
                        background: offer.candidates.length > 0 ? accentColor : undefined,
                        opacity: offer.candidates.length === 0 ? 0.4 : 1,
                    }}
                    disabled={offer.candidates.length === 0}
                    onClick={() => onStartNegotiation(offer.id)}
                >
                    Bắt đầu đàm phán
                </button>
                <button
                    className="fmc-btn-cancel"
                    onClick={() => onCancelOffer(offer.id)}
                >
                    Hủy
                </button>
            </div>
        </div>
    );
}

export function FindMatchingView({ offers, onStartNegotiation, onCancelOffer }: FindMatchingViewProps) {
    if (offers.length === 0) {
        return (
            <div className="find-matching-empty">
                <div className="fme-icon">
                    <svg width="48" height="48" viewBox="0 0 48 48" fill="none">
                        <circle cx="24" cy="24" r="20" stroke="#4DA6FF" strokeWidth="2" strokeDasharray="4 4" />
                        <path d="M16 24h16M24 16v16" stroke="#4DA6FF" strokeWidth="2" strokeLinecap="round" />
                    </svg>
                </div>
                <p className="fme-text">Chưa có yêu cầu nào đang tìm đối tác</p>
                <p className="fme-sub">Tạo yêu cầu mua/bán ở tab "Create Request" và nhấn "Bắt đầu tìm kiếm"</p>
            </div>
        );
    }

    return (
        <div className="find-matching-view">
            <div className="fmv-header">
                <span className="fmv-title">Đang tìm đối tác</span>
                <span className="fmv-count">{offers.length}</span>
            </div>
            <div className="fmv-list">
                {offers.map(offer => (
                    <OfferCard
                        key={offer.id}
                        offer={offer}
                        onStartNegotiation={onStartNegotiation}
                        onCancelOffer={onCancelOffer}
                    />
                ))}
            </div>
        </div>
    );
}
