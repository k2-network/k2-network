/**
 * Tab System for Marketplace
 * Tab 1: Discover Deals - Browse existing offers
 * Tab 2: Create Request - Dynamic form for creating new requests
 * Tab 3: Finding Match - P2P discovery in progress
 * Tab 4: Negotiation - AI negotiation dashboard
 */
import React from 'react';
import { IoSearch, IoAddCircle, IoRadio, IoChatbubbles } from 'react-icons/io5';
import './DynamicForm.css';

export type TabType = 'discover' | 'create' | 'finding' | 'negotiation';

interface MarketplaceTabsProps {
    activeTab: TabType;
    onTabChange: (tab: TabType) => void;
}

export const MarketplaceTabs: React.FC<MarketplaceTabsProps> = ({
    activeTab,
    onTabChange
}) => {
    return (
        <div className="form-tabs">
            <button
                className={`form-tab ${activeTab === 'discover' ? 'active' : ''}`}
                onClick={() => onTabChange('discover')}
            >
                <span className="form-tab-icon">
                    <IoSearch />
                </span>
                Discover Deals
            </button>
            <button
                className={`form-tab ${activeTab === 'create' ? 'active' : ''}`}
                onClick={() => onTabChange('create')}
            >
                <span className="form-tab-icon">
                    <IoAddCircle />
                </span>
                Create Request
            </button>
            <button
                className={`form-tab ${activeTab === 'finding' ? 'active' : ''}`}
                onClick={() => onTabChange('finding')}
            >
                <span className="form-tab-icon">
                    <IoRadio />
                </span>
                Finding Match
            </button>
            <button
                className={`form-tab ${activeTab === 'negotiation' ? 'active' : ''}`}
                onClick={() => onTabChange('negotiation')}
            >
                <span className="form-tab-icon">
                    <IoChatbubbles />
                </span>
                Negotiation
            </button>
        </div>
    );
};

export default MarketplaceTabs;
