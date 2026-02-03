/**
 * Tambo Generative Components Registration
 * 
 * Register React components that Tambo can dynamically render
 */
import { z } from "zod";
import type { TamboComponent } from "@tambo-ai/react";
import { DynamicRequestForm } from "../components/DynamicForm";

// ============================================
// SCHEMA LỎng - dùng z.string() thay vì z.enum() 
// để tránh validation error từ Tambo SDK
// ============================================

const dynamicFormPropsSchema = z.object({
    // TẤT CẢ props đều optional để tránh validation error
    topic: z.string().optional().describe("Danh mục: Digital Assets, Goods, hoặc Freelance Job"),
    action: z.string().optional().describe("Hành động: buy, sell, exchange"),
    title: z.string().optional().describe("Tiêu đề yêu cầu"),
    description: z.string().optional().describe("Mô tả chi tiết"),
    subtopic: z.string().optional().describe("Danh mục con cho Digital Assets hoặc Goods"),
    category: z.string().optional().describe("Category cho Freelance Job"),
    skill: z.string().optional().describe("Skill cụ thể cho Freelance Job"),
    priceMin: z.number().optional().describe("Giá tối thiểu (USD)"),
    priceMax: z.number().optional().describe("Giá tối đa (USD)"),
    brand: z.string().optional().describe("Thương hiệu sản phẩm"),
    location: z.string().optional().describe("Vị trí/khu vực giao dịch"),
    condition: z.string().optional().describe("Tình trạng: new, used, like-new, any"),
});

// Normalize topic value
const normalizeTopic = (topic?: string): "Digital Assets" | "Goods" | "Freelance Job" => {
    if (!topic) return "Goods";
    const lower = topic.toLowerCase();
    if (lower.includes("digital") || lower.includes("video") || lower.includes("audio") || lower.includes("tài sản số")) {
        return "Digital Assets";
    }
    if (lower.includes("freelance") || lower.includes("job") || lower.includes("việc làm")) {
        return "Freelance Job";
    }
    return "Goods"; // Default
};

// Normalize action value  
const normalizeAction = (action?: string): "buy" | "sell" | "exchange" => {
    if (!action) return "buy";
    const lower = action.toLowerCase();
    if (lower.includes("sell") || lower.includes("bán")) return "sell";
    if (lower.includes("exchange") || lower.includes("trao đổi")) return "exchange";
    return "buy"; // Default
};

// Normalize condition value
const normalizeCondition = (condition?: string): "new" | "used" | "like-new" | "any" => {
    if (!condition) return "any";
    const lower = condition.toLowerCase();
    if (lower.includes("new") && !lower.includes("like")) return "new";
    if (lower.includes("like-new") || lower.includes("like new")) return "like-new";
    if (lower.includes("used") || lower.includes("cũ")) return "used";
    return "any";
};

// Wrapper component với normalization
const DynamicFormWrapper: React.FC<z.infer<typeof dynamicFormPropsSchema>> = (props) => {
    console.log('📥 [DynamicFormWrapper] Raw props from AI:', props);
    
    // Normalize values
    const normalizedTopic = normalizeTopic(props.topic);
    const normalizedAction = normalizeAction(props.action);
    const normalizedCondition = normalizeCondition(props.condition);
    
    console.log('✅ [DynamicFormWrapper] Normalized:', { 
        topic: normalizedTopic, 
        action: normalizedAction,
        condition: normalizedCondition 
    });

    // Build selection based on topic
    const selection = normalizedTopic === "Freelance Job"
        ? { category: props.category || "", skill: props.skill || "" }
        : { subtopic: props.subtopic || "" };

    // Transform to DynamicRequestForm format
    const formData = {
        topic: normalizedTopic,
        action: normalizedAction,
        selection,
        title: props.title,
        description: props.description,
        priceRange: {
            min: props.priceMin || 0,
            max: props.priceMax || 10000,
            currency: "USD"
        },
        location: props.location || "",
        condition: normalizedCondition,
        ...(props.brand ? { brand: props.brand } : {})
    };

    const handleSubmit = (data: any) => {
        console.log("📤 [DynamicForm] Submitted:", data);
        window.dispatchEvent(new CustomEvent('k2:formSubmitted', { detail: data }));
    };

    return (
        <DynamicRequestForm
            initialData={formData}
            onSubmit={handleSubmit}
            isStreaming={false}
        />
    );
};

// Register DynamicRequestForm as TamboComponent
export const DynamicRequestFormComponent: TamboComponent = {
    name: "DynamicRequestForm",
    description: `Form động để tạo yêu cầu mua/bán/trao đổi trên K2 Marketplace.

QUAN TRỌNG - ĐIỀU KIỆN SỬ DỤNG:
- CHỈ render SAU KHI người dùng XÁC NHẬN (nói "có", "ok", "tạo đi", "được")
- KHÔNG render ngay khi người dùng mới nói muốn mua/bán

PROPS CẦN TRUYỀN:
- topic: "Digital Assets" hoặc "Goods" hoặc "Freelance Job"
- action: "buy" hoặc "sell" hoặc "exchange"
- title: Tiêu đề yêu cầu
- description: Mô tả chi tiết
- subtopic: Danh mục con (Video, Electronics, etc.)`,
    component: DynamicFormWrapper,
    propsSchema: dynamicFormPropsSchema,
};

// Export all Tambo components
export const tamboComponents: TamboComponent[] = [
    DynamicRequestFormComponent,
];
