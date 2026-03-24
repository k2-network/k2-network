/**
 * Skeleton Field Component - For loading state
 */
import React from 'react';

interface SkeletonFieldProps {
    type?: 'input' | 'textarea' | 'text' | 'select';
    width?: string;
}

export const SkeletonField: React.FC<SkeletonFieldProps> = ({ type = 'input', width }) => {
    const getClassName = () => {
        switch (type) {
            case 'textarea':
                return 'skeleton-loader skeleton-textarea';
            case 'text':
                return 'skeleton-loader skeleton-text';
            case 'select':
                return 'skeleton-loader skeleton-input';
            default:
                return 'skeleton-loader skeleton-input';
        }
    };

    return <div className={getClassName()} style={{ width }} />;
};

interface SkeletonFormProps {
    topic?: 'Goods' | 'Digital Assets' | 'Freelance Job';
}

export const SkeletonForm: React.FC<SkeletonFormProps> = ({ topic: _topic }) => {
    return (
        <div className="dynamic-form-container">
            {/* Header Skeleton */}
            <div className="form-header">
                <div className="skeleton-loader" style={{ width: 48, height: 48, borderRadius: 12 }} />
                <div style={{ flex: 1 }}>
                    <div className="skeleton-loader skeleton-text short" />
                    <div className="skeleton-loader skeleton-text medium" style={{ height: 12, marginTop: 8 }} />
                </div>
            </div>

            {/* Action Toggle Skeleton */}
            <div className="action-toggle" style={{ opacity: 0.5 }}>
                <div className="skeleton-loader" style={{ flex: 1, height: 40, borderRadius: 6 }} />
                <div className="skeleton-loader" style={{ flex: 1, height: 40, borderRadius: 6 }} />
                <div className="skeleton-loader" style={{ flex: 1, height: 40, borderRadius: 6 }} />
            </div>

            {/* Form Fields Skeleton */}
            <div className="form-section">
                <div className="skeleton-loader skeleton-text short" style={{ height: 12, marginBottom: 16 }} />
                <div className="form-row">
                    <div className="form-field">
                        <div className="skeleton-loader skeleton-text" style={{ width: 60, height: 10, marginBottom: 8 }} />
                        <SkeletonField type="input" />
                    </div>
                    <div className="form-field">
                        <div className="skeleton-loader skeleton-text" style={{ width: 80, height: 10, marginBottom: 8 }} />
                        <SkeletonField type="input" />
                    </div>
                </div>
            </div>

            <div className="form-section">
                <div className="skeleton-loader skeleton-text short" style={{ height: 12, marginBottom: 16 }} />
                <SkeletonField type="textarea" />
            </div>

            {/* Price Range Skeleton */}
            <div className="form-section">
                <div className="skeleton-loader skeleton-text short" style={{ height: 12, marginBottom: 16 }} />
                <div className="price-range-container" style={{ opacity: 0.5 }}>
                    <div className="skeleton-loader" style={{ height: 6, borderRadius: 3, marginBottom: 12 }} />
                    <div className="price-inputs">
                        <div className="skeleton-loader" style={{ flex: 1, height: 36, borderRadius: 6 }} />
                        <div className="skeleton-loader" style={{ flex: 1, height: 36, borderRadius: 6 }} />
                    </div>
                </div>
            </div>

            {/* Submit Button Skeleton */}
            <div className="form-actions">
                <div className="skeleton-loader" style={{ flex: 1, height: 48, borderRadius: 8 }} />
            </div>
        </div>
    );
};

export default SkeletonField;
