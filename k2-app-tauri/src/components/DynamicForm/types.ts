/**
 * Types for Dynamic Request Form
 */

export type TopicType = 'Digital Assets' | 'Goods' | 'Freelance Job';
export type ActionType = 'buy' | 'sell' | 'exchange';

// Selection types based on topic
export interface DigitalAssetsSelection {
    subtopic: string; // Video, Audio, Image, Document, Software, Other
}

export interface GoodsSelection {
    subtopic: string; // Electronics & Devices, Fashion & Accessories, Home & Living, etc.
}

export interface FreelanceJobSelection {
    category: string; // Development, Design, Marketing, Writing, etc.
    skill: string; // specific skill
}

export type TopicSelection = DigitalAssetsSelection | GoodsSelection | FreelanceJobSelection;

// Common fields for all forms
export interface CommonFormFields {
    topic: TopicType;
    selection: TopicSelection;
    action: ActionType;
    title: string;
    description: string;
    priceRange: {
        min: number;
        max: number;
        currency: string;
    };
    location: string;
    condition: 'new' | 'used' | 'like-new' | 'any';
}

// Goods-specific fields
export interface GoodsFormFields extends CommonFormFields {
    topic: 'Goods';
    brand?: string;
    model?: string;
    warranty?: boolean;
    shippingMethod?: 'pickup' | 'delivery' | 'both';
}

// Digital Assets-specific fields  
export interface DigitalAssetsFormFields extends CommonFormFields {
    topic: 'Digital Assets';
    fileFormat?: string;
    resolution?: string;
    duration?: string; // for video/audio
    license?: 'personal' | 'commercial' | 'exclusive';
}

// Freelance Job-specific fields
export interface FreelanceJobFormFields extends CommonFormFields {
    topic: 'Freelance Job';
    deadline?: string;
    experienceLevel?: 'entry' | 'intermediate' | 'expert';
    projectType?: 'one-time' | 'ongoing' | 'contract';
    deliverables?: string[];
}

export type DynamicFormFields = GoodsFormFields | DigitalAssetsFormFields | FreelanceJobFormFields;

// Form field loading state
export interface FieldLoadingState {
    topic: boolean;
    selection: boolean;
    action: boolean;
    title: boolean;
    description: boolean;
    priceRange: boolean;
    location: boolean;
    condition: boolean;
    // Topic-specific
    brand?: boolean;
    fileFormat?: boolean;
    deadline?: boolean;
    [key: string]: boolean | undefined;
}

// Props for DynamicRequestForm component
export interface DynamicRequestFormProps {
    initialData?: Partial<DynamicFormFields>;
    onSubmit?: (data: DynamicFormFields) => void;
    onCancel?: () => void;
    isStreaming?: boolean;
    streamingFields?: Partial<FieldLoadingState>;
}
