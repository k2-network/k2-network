/**
 * Tab System for Marketplace
 * Tab 1: Discover Deals - Browse existing offers
 * Tab 2: Create Request - Dynamic form for creating new requests
 */
import React from 'react';
import { IoSearch, IoAddCircle } from 'react-icons/io5';
import './DynamicForm.css';

export type TabType = 'discover' | 'create';

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
        </div>
    );
};

export default MarketplaceTabs;
