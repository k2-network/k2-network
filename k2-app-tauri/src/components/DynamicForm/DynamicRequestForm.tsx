/**
 * Dynamic Request Form Component
 * Renders different form layouts based on topic type
 * Supports streaming/loading states for AI-generated content
 */
import React, { useState, useEffect } from 'react';
import { IoSearch } from 'react-icons/io5';
import { SkeletonField, SkeletonForm } from './SkeletonField';
import type {
    DynamicFormFields,
    TopicType,
    GoodsFormFields,
    DigitalAssetsFormFields,
    FreelanceJobFormFields,
    FieldLoadingState
} from './types';
import './DynamicForm.css';

// Topic configurations
const TOPIC_CONFIG = {
    'Goods': {
        icon: '📦',
        gradient: 'goods',
        title: 'Goods Request',
        description: 'Mua bán trao đổi hàng hóa vật lý',
        subtopics: [
            'Electronics & Devices',
            'Fashion & Accessories',
            'Home & Living',
            'Sports & Outdoors',
            'Books & Media',
            'Collectibles',
            'Other'
        ]
    },
    'Digital Assets': {
        icon: '🎬',
        gradient: 'digital',
        title: 'Digital Assets Request',
        description: 'Tài sản số: video, audio, hình ảnh, phần mềm',
        subtopics: [
            'Video',
            'Audio',
            'Image',
            'Document',
            'Software',
            '3D Model',
            'Other'
        ]
    },
    'Freelance Job': {
        icon: '💼',
        gradient: 'freelance',
        title: 'Freelance Job Request',
        description: 'Tìm kiếm hoặc cung cấp dịch vụ freelance',
        categories: [
            'Development',
            'Design',
            'Marketing',
            'Writing',
            'Video & Animation',
            'Music & Audio',
            'Business',
            'Other'
        ]
    }
};

interface DynamicRequestFormProps {
    initialData?: Partial<DynamicFormFields>;
    onSubmit?: (data: DynamicFormFields) => void;
    onCancel?: () => void;
    isStreaming?: boolean;
    streamingFields?: Partial<FieldLoadingState>;
}

export const DynamicRequestForm: React.FC<DynamicRequestFormProps> = ({
    initialData,
    onSubmit,
    onCancel,
    isStreaming = false,
    streamingFields = {}
}) => {
    const [formData, setFormData] = useState<Partial<DynamicFormFields>>({
        topic: 'Goods',
        action: 'buy',
        title: '',
        description: '',
        priceRange: { min: 0, max: 1000, currency: 'USD' },
        location: '',
        condition: 'any',
        ...initialData
    });

    const [loadedFields, setLoadedFields] = useState<Set<string>>(new Set());

    // Simulate streaming field loading
    useEffect(() => {
        if (initialData && isStreaming) {
            const fields = Object.keys(initialData);
            let index = 0;

            const interval = setInterval(() => {
                if (index < fields.length) {
                    setLoadedFields(prev => new Set([...prev, fields[index]]));
                    index++;
                } else {
                    clearInterval(interval);
                }
            }, 200);

            return () => clearInterval(interval);
        } else if (initialData) {
            setLoadedFields(new Set(Object.keys(initialData)));
        }
    }, [initialData, isStreaming]);

    // Update form when initialData changes (from AI)
    useEffect(() => {
        if (initialData) {
            setFormData(prev => ({ ...prev, ...initialData }));
        }
    }, [initialData]);

    const topic = formData.topic as TopicType;
    const config = TOPIC_CONFIG[topic];

    const handleFieldChange = (field: string, value: any) => {
        setFormData(prev => ({ ...prev, [field]: value }));
    };

    const handlePriceChange = (type: 'min' | 'max', value: number) => {
        setFormData(prev => ({
            ...prev,
            priceRange: {
                ...prev.priceRange!,
                [type]: value
            }
        }));
    };

    const handleSubmit = () => {
        if (onSubmit) {
            onSubmit(formData as DynamicFormFields);
        }
    };

    const isFieldLoading = (field: string) => {
        if (!isStreaming) return false;
        return streamingFields[field] === true || !loadedFields.has(field);
    };

    // Show skeleton form if topic is still loading
    if (isStreaming && !loadedFields.has('topic')) {
        return <SkeletonForm />;
    }

    return (
        <div className="dynamic-form-container">
            {/* Streaming Indicator */}
            {isStreaming && (
                <div className="streaming-indicator">
                    <div className="streaming-dot" />
                    <span className="streaming-text">AI đang điền thông tin...</span>
                </div>
            )}

            {/* Header */}
            <div className="form-header">
                <div className={`form-icon ${config.gradient}`}>
                    {config.icon}
                </div>
                <div className="form-header-text">
                    <h3>{config.title}</h3>
                    <p>{config.description}</p>
                </div>
            </div>

            {/* Action Toggle */}
            <div className="action-toggle">
                <button
                    className={`action-btn ${formData.action === 'buy' ? 'active buy' : ''}`}
                    onClick={() => handleFieldChange('action', 'buy')}
                >
                    Mua
                </button>
                <button
                    className={`action-btn ${formData.action === 'sell' ? 'active sell' : ''}`}
                    onClick={() => handleFieldChange('action', 'sell')}
                >
                    Bán
                </button>
                <button
                    className={`action-btn ${formData.action === 'exchange' ? 'active exchange' : ''}`}
                    onClick={() => handleFieldChange('action', 'exchange')}
                >
                    Trao đổi
                </button>
            </div>

            {/* Basic Information Section */}
            <div className="form-section">
                <div className="form-section-title">Thông tin cơ bản</div>

                <div className="form-row">
                    <div className={`form-field ${isFieldLoading('title') ? 'loading' : ''}`}>
                        <label className="form-label">
                            Tiêu đề
                        </label>
                        {isFieldLoading('title') ? (
                            <SkeletonField type="input" />
                        ) : (
                            <input
                                type="text"
                                className="form-input"
                                placeholder="Nhập tiêu đề yêu cầu..."
                                value={formData.title || ''}
                                onChange={(e) => handleFieldChange('title', e.target.value)}
                            />
                        )}
                    </div>
                </div>

                <div className="form-row">
                    <div className={`form-field ${isFieldLoading('description') ? 'loading' : ''}`}>
                        <label className="form-label">
                            Mô tả chi tiết
                        </label>
                        {isFieldLoading('description') ? (
                            <SkeletonField type="textarea" />
                        ) : (
                            <textarea
                                className="form-input form-textarea"
                                placeholder="Mô tả chi tiết yêu cầu của bạn..."
                                value={formData.description || ''}
                                onChange={(e) => handleFieldChange('description', e.target.value)}
                            />
                        )}
                    </div>
                </div>
            </div>

            {/* Category/Subtopic Section */}
            <div className="form-section">
                <div className="form-section-title">Danh mục</div>

                <div className="tag-container">
                    {topic === 'Freelance Job' ? (
                        // Freelance categories
                        TOPIC_CONFIG['Freelance Job'].categories?.map((cat: string) => {
                            const currentSelection = formData.selection as { category?: string; skill?: string } | undefined;
                            return (
                                <button
                                    key={cat}
                                    className={`tag ${currentSelection?.category === cat ? 'selected' : ''}`}
                                    onClick={() => handleFieldChange('selection', { ...formData.selection, category: cat })}
                                >
                                    <span className="tag-prefix">#</span> {cat}
                                </button>
                            );
                        })
                    ) : (
                        // Goods or Digital Assets subtopics
                        ('subtopics' in config ? config.subtopics : []).map((sub: string) => (
                            <button
                                key={sub}
                                className={`tag ${(formData.selection as any)?.subtopic === sub ? 'selected' : ''}`}
                                onClick={() => handleFieldChange('selection', { subtopic: sub })}
                            >
                                <span className="tag-prefix">#</span> {sub}
                            </button>
                        ))
                    )}
                </div>
            </div>

            {/* Topic-Specific Fields */}
            {topic === 'Goods' && (
                <div className="form-section">
                    <div className="form-section-title">Chi tiết sản phẩm</div>
                    <div className="form-row">
                        <div className={`form-field ${isFieldLoading('brand') ? 'loading' : ''}`}>
                            <label className="form-label">Thương hiệu</label>
                            {isFieldLoading('brand') ? (
                                <SkeletonField type="input" />
                            ) : (
                                <input
                                    type="text"
                                    className="form-input"
                                    placeholder="VD: Apple, Samsung, Sony..."
                                    value={(formData as GoodsFormFields).brand || ''}
                                    onChange={(e) => handleFieldChange('brand', e.target.value)}
                                />
                            )}
                        </div>
                        <div className="form-field">
                            <label className="form-label">Model</label>
                            <input
                                type="text"
                                className="form-input"
                                placeholder="VD: iPhone 15 Pro, Galaxy S24..."
                                value={(formData as GoodsFormFields).model || ''}
                                onChange={(e) => handleFieldChange('model', e.target.value)}
                            />
                        </div>
                    </div>

                    {/* Condition */}
                    <div className="form-row" style={{ marginTop: 12 }}>
                        <div className="form-field">
                            <label className="form-label">Tình trạng</label>
                            <div className="condition-options">
                                {['new', 'like-new', 'used', 'any'].map((cond) => (
                                    <button
                                        key={cond}
                                        className={`condition-option ${formData.condition === cond ? 'selected' : ''}`}
                                        onClick={() => handleFieldChange('condition', cond)}
                                    >
                                        {cond === 'new' ? 'Mới' : cond === 'like-new' ? 'Như mới' : cond === 'used' ? 'Đã dùng' : 'Bất kỳ'}
                                    </button>
                                ))}
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {topic === 'Digital Assets' && (
                <div className="form-section">
                    <div className="form-section-title">Chi tiết tài sản số</div>
                    <div className="form-row">
                        <div className="form-field">
                            <label className="form-label">Định dạng file</label>
                            <select
                                className="form-input form-select"
                                value={(formData as DigitalAssetsFormFields).fileFormat || ''}
                                onChange={(e) => handleFieldChange('fileFormat', e.target.value)}
                            >
                                <option value="">Chọn định dạng...</option>
                                <option value="mp4">MP4</option>
                                <option value="mov">MOV</option>
                                <option value="mp3">MP3</option>
                                <option value="wav">WAV</option>
                                <option value="png">PNG</option>
                                <option value="psd">PSD</option>
                                <option value="other">Khác</option>
                            </select>
                        </div>
                        <div className="form-field">
                            <label className="form-label">Độ phân giải / Chất lượng</label>
                            <input
                                type="text"
                                className="form-input"
                                placeholder="VD: 4K, 1080p, 320kbps..."
                                value={(formData as DigitalAssetsFormFields).resolution || ''}
                                onChange={(e) => handleFieldChange('resolution', e.target.value)}
                            />
                        </div>
                    </div>
                    <div className="form-row">
                        <div className="form-field">
                            <label className="form-label">Loại license</label>
                            <div className="condition-options">
                                {['personal', 'commercial', 'exclusive'].map((lic) => (
                                    <button
                                        key={lic}
                                        className={`condition-option ${(formData as DigitalAssetsFormFields).license === lic ? 'selected' : ''}`}
                                        onClick={() => handleFieldChange('license', lic)}
                                    >
                                        {lic === 'personal' ? 'Cá nhân' : lic === 'commercial' ? 'Thương mại' : 'Độc quyền'}
                                    </button>
                                ))}
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {topic === 'Freelance Job' && (
                <div className="form-section">
                    <div className="form-section-title">Chi tiết công việc</div>
                    <div className="form-row">
                        <div className="form-field">
                            <label className="form-label">Deadline</label>
                            <input
                                type="date"
                                className="form-input"
                                value={(formData as FreelanceJobFormFields).deadline || ''}
                                onChange={(e) => handleFieldChange('deadline', e.target.value)}
                            />
                        </div>
                        <div className="form-field">
                            <label className="form-label">Level yêu cầu</label>
                            <div className="condition-options">
                                {['entry', 'intermediate', 'expert'].map((level) => (
                                    <button
                                        key={level}
                                        className={`condition-option ${(formData as FreelanceJobFormFields).experienceLevel === level ? 'selected' : ''}`}
                                        onClick={() => handleFieldChange('experienceLevel', level)}
                                    >
                                        {level === 'entry' ? 'Entry' : level === 'intermediate' ? 'Mid' : 'Expert'}
                                    </button>
                                ))}
                            </div>
                        </div>
                    </div>
                    <div className="form-row">
                        <div className="form-field">
                            <label className="form-label">Loại dự án</label>
                            <div className="condition-options">
                                {['one-time', 'ongoing', 'contract'].map((type) => (
                                    <button
                                        key={type}
                                        className={`condition-option ${(formData as FreelanceJobFormFields).projectType === type ? 'selected' : ''}`}
                                        onClick={() => handleFieldChange('projectType', type)}
                                    >
                                        {type === 'one-time' ? 'Một lần' : type === 'ongoing' ? 'Dài hạn' : 'Hợp đồng'}
                                    </button>
                                ))}
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* Price Range Section */}
            <div className="form-section">
                <div className="form-section-title">Ngân sách</div>
                <div className="price-range-container">
                    <div className="price-range-header">
                        <span className="form-label">Khoảng giá</span>
                        <span className="price-value">
                            ${formData.priceRange?.min?.toLocaleString()} - ${formData.priceRange?.max?.toLocaleString()}
                        </span>
                    </div>
                    <div className="price-inputs">
                        <div className="price-input-group">
                            <input
                                type="number"
                                value={formData.priceRange?.min || 0}
                                onChange={(e) => handlePriceChange('min', Number(e.target.value))}
                                placeholder="Min"
                            />
                            <span>USD</span>
                        </div>
                        <div className="price-input-group">
                            <input
                                type="number"
                                value={formData.priceRange?.max || 0}
                                onChange={(e) => handlePriceChange('max', Number(e.target.value))}
                                placeholder="Max"
                            />
                            <span>USD</span>
                        </div>
                    </div>
                </div>
            </div>

            {/* Location */}
            <div className="form-section">
                <div className="form-section-title">Vị trí</div>
                <div className="form-row">
                    <div className="form-field">
                        <label className="form-label">
                            Khu vực
                        </label>
                        <input
                            type="text"
                            className="form-input"
                            placeholder="VD: Hà Nội, HCM, Online..."
                            value={formData.location || ''}
                            onChange={(e) => handleFieldChange('location', e.target.value)}
                        />
                    </div>
                </div>
            </div>

            {/* Form Actions */}
            <div className="form-actions">
                {onCancel && (
                    <button className="btn-cancel" onClick={onCancel}>
                        Hủy
                    </button>
                )}
                <button
                    className="btn-submit"
                    onClick={handleSubmit}
                    disabled={isStreaming || !formData.title}
                >
                    <IoSearch /> Bắt đầu tìm kiếm
                </button>
            </div>
        </div>
    );
};

export default DynamicRequestForm;
