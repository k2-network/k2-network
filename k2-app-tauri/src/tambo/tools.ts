/**
 * Tambo Tools for K2 Marketplace
 * 
 * Tools để Tambo AI có thể gọi và thực hiện các action
 */
import { z } from "zod";
// import { extractMarketplaceIntent } from "../services/groqStructuredOutput";

/**
 * Alternative marketplace intent extraction using K2 DigitalOcean endpoint
 */
/**
 * Alternative marketplace intent extraction using K2 DigitalOcean endpoint
 * UPDATED: Uses Tauri backend to bypass SSL cert errors
 */
async function extractMarketplaceIntentK2(userPrompt: string): Promise<any> {
    console.log("🔍 [K2-DO] Calling backend classify_k2_endpoint (ignoring SSL)...");

    // Import Tauri invoke
    const { invoke } = await import('@tauri-apps/api/core');

    // Call backend command
    const rawResult = await invoke<any>('classify_k2_endpoint', {
        userPrompt
    });

    console.log("✅ [K2-DO] Backend returned raw result:", rawResult);

    let data = rawResult;
    // Handle double-encoded JSON if happens
    if (typeof data === 'string') {
        try {
            data = JSON.parse(data);
        } catch (e) {
            console.error("Failed to parse string data:", e);
        }
    }

    // Transform K2 response to match groq format
    // K2 returns: { action: { topic: "...", ... }, action_type: "Sell" }
    // or just check if it has the right structure
    if (!data || (!data.action && !data.topic)) {
        throw new Error(`Invalid K2 response structure: ${JSON.stringify(data)}`);
    }

    // If already in correct format (rare but possible if endpoint changes)
    if (data.topic && data.selection && data.action) {
        return data;
    }

    // Map K2 format to App format
    const actionData = data.action || {};
    const topic = actionData.topic || "Goods"; // default

    const result = {
        topic: topic,
        selection: topic === "Freelance Job"
            ? {
                category: actionData.category || "Tech & IT",
                skill: actionData.skill || "Web & Mobile Development"
            }
            : { subtopic: actionData.subtopic || "Other" },
        action: (data.action_type || "sell").toLowerCase(),
        description: userPrompt
    };

    console.log("✨ [K2-DO] Transformed result:", result);
    return result;
}

/**
 * Extract marketplace intent using Tauri backend (no CORS issues)
 */
async function extractMarketplaceIntentTauri(userPrompt: string): Promise<any> {
    console.log("🔧 [Tauri] Calling backend classify_intent...");

    const { invoke } = await import('@tauri-apps/api/core');
    const apiKey = import.meta.env.VITE_GROQ_API_KEY;
    const baseUrl = import.meta.env.VITE_GROQ_BASE_URL || 'https://api.groq.com/openai/v1';
    const model = import.meta.env.VITE_GROQ_SMALL_MODEL || 'llama-3.3-70b-versatile';

    if (!apiKey) {
        throw new Error('VITE_GROQ_API_KEY is not configured');
    }

    const result = await invoke<any>('classify_intent', {
        userPrompt,
        apiKey,
        baseUrl,
        model
    });

    console.log("✅ [Tauri] Backend result:", result);
    return result;
}

/**
 * Parallel execution: Tauri backend first, then K2 endpoint fallback
 */
async function extractMarketplaceIntentParallel(userPrompt: string) {
    console.log("🚀 [Parallel] Starting extraction via Tauri backend...");

    // Try Tauri backend first (no CORS)
    try {
        const result = await extractMarketplaceIntentTauri(userPrompt);
        console.log("✅ [Parallel] Using Tauri backend result");
        return result;
    } catch (tauriError) {
        console.warn("⚠️ [Parallel] Tauri backend failed:", tauriError);
    }

    // Fallback to K2 endpoint
    try {
        const k2Result = await extractMarketplaceIntentK2(userPrompt);
        console.log("✅ [Parallel] Using K2 endpoint result");
        return k2Result;
    } catch (k2Error) {
        console.error("❌ [Parallel] K2 endpoint also failed:", k2Error);
    }

    throw new Error("All classification methods failed");
}


/**
 * Tool: Extract Marketplace Intent
 * 
 * Khi người dùng muốn mua/bán/trao đổi, tool này sẽ:
 * 1. Nhận prompt từ người dùng
 * 2. Gọi Groq với strict mode structured output
 * 3. Trả về JSON với thông tin chi tiết
 */
const extractMarketplaceIntentTool = {
    name: "extract-marketplace-intent",
    description: `Phân tích yêu cầu mua/bán/trao đổi của người dùng trên K2 Marketplace.
    
Sử dụng khi người dùng:
- Muốn MUA sản phẩm/dịch vụ (ví dụ: "mua video", "cần thuê developer")
- Muốn BÁN sản phẩm/dịch vụ (ví dụ: "bán source code", "cung cấp dịch vụ design")
- Muốn TRAO ĐỔI (ví dụ: "đổi laptop", "exchange token")

Tool sẽ trả về JSON structured với:
- topic: Danh mục chính (Digital Assets, Goods, Freelance Job)
- selection: Chi tiết danh mục con
- action: buy/sell/exchange
- description: Mô tả yêu cầu`,

    tool: async (input: { userPrompt: string }): Promise<string> => {
        console.log("[K2 Tool] extractMarketplaceIntent START", { input });

        try {
            const result = await extractMarketplaceIntentParallel(input.userPrompt);
            console.log("[K2 Tool] extractMarketplaceIntent SUCCESS", result);

            // Format response for user
            const actionLabels: Record<string, string> = {
                buy: "MUA",
                sell: "BÁN",
                exchange: "TRAO ĐỔI"
            };

            let selectionInfo = "";
            if ("subtopic" in result.selection) {
                selectionInfo = `**Danh mục:** ${result.topic} > ${result.selection.subtopic}`;
            } else {
                selectionInfo = `**Danh mục:** ${result.topic} > ${result.selection.category} > ${result.selection.skill}`;
            }

            const response = `
**Đã phân tích yêu cầu của bạn!**

**Hành động:** ${actionLabels[result.action]},
${selectionInfo}

---
**Dữ liệu JSON:**
\`\`\`json
${JSON.stringify(result, null, 2)}
\`\`\`

Bạn có muốn tôi tạo yêu cầu giao dịch này không?
`;
            return response;

        } catch (error) {
            console.error("🔥 [K2 Tool] extractMarketplaceIntent ERROR:", error);
            return `❌ Không thể phân tích yêu cầu: ${error instanceof Error ? error.message : "Unknown error"}`;
        }
    },

    inputSchema: z.object({
        userPrompt: z.string().describe("Yêu cầu của người dùng về mua/bán/trao đổi")
    }),

    outputSchema: z.string().describe("Kết quả phân tích với JSON structured")
};

/**
 * Tool: Create Trade Request
 * 
 * Tạo yêu cầu giao dịch sau khi đã phân tích intent
 */
const createTradeRequestTool = {
    name: "create-trade-request",
    description: `Tạo yêu cầu giao dịch mới trên K2 Marketplace.
    
Sử dụng sau khi đã phân tích intent và người dùng xác nhận muốn tạo yêu cầu.`,

    tool: async (input: {
        topic: string;
        subtopic?: string;
        category?: string;
        skill?: string;
        action: string;
        title: string;
        description: string;
        price?: number;
    }): Promise<string> => {
        console.log("🚀 [K2 Tool] createTradeRequest START", { input });

        try {
            // TODO: Implement actual trade request creation via P2P network
            // For now, return mock success
            const requestId = `K2-${Date.now().toString(36).toUpperCase()}`;

            const response = `
✅ **Đã tạo yêu cầu giao dịch thành công!**

**ID:** ${requestId}
**Tiêu đề:** ${input.title}
**Loại:** ${input.action.toUpperCase()}
**Danh mục:** ${input.topic} > ${input.subtopic || `${input.category} > ${input.skill}`}
**Mô tả:** ${input.description}
${input.price ? `**Giá:** ${input.price.toLocaleString()} VND` : "**Giá:** Thương lượng"}

📢 Yêu cầu của bạn đã được phát lên mạng P2P. Bạn sẽ nhận được thông báo khi có người quan tâm.
`;
            return response;

        } catch (error) {
            console.error("🔥 [K2 Tool] createTradeRequest ERROR:", error);
            return `❌ Không thể tạo yêu cầu: ${error instanceof Error ? error.message : "Unknown error"}`;
        }
    },

    inputSchema: z.object({
        topic: z.string().describe("Danh mục chính: Digital Assets, Goods, hoặc Freelance Job"),
        subtopic: z.string().optional().describe("Danh mục con cho Digital Assets và Goods"),
        category: z.string().optional().describe("Category cho Freelance Job"),
        skill: z.string().optional().describe("Skill cụ thể cho Freelance Job"),
        action: z.enum(["buy", "sell", "exchange"]).describe("Loại giao dịch"),
        title: z.string().describe("Tiêu đề yêu cầu"),
        description: z.string().describe("Mô tả chi tiết"),
        price: z.number().optional().describe("Giá đề xuất (VND)")
    }),

    outputSchema: z.string().describe("Kết quả tạo yêu cầu")
};

/**
 * Tool: Search Marketplace
 * 
 * Tìm kiếm sản phẩm/dịch vụ trên marketplace
 */
const searchMarketplaceTool = {
    name: "search-marketplace",
    description: `Tìm kiếm sản phẩm hoặc dịch vụ trên K2 Marketplace.
    
Sử dụng khi người dùng muốn tìm kiếm, xem danh sách, hoặc khám phá marketplace.`,

    tool: async (input: {
        query: string;
        topic?: string;
    }): Promise<string> => {
        console.log("🚀 [K2 Tool] searchMarketplace START", { input });

        try {
            // TODO: Implement actual search via P2P network
            // For now, return mock results
            const response = `
🔍 **Kết quả tìm kiếm: "${input.query}"**
${input.topic ? `(Trong danh mục: ${input.topic})` : ""}

**Không tìm thấy kết quả phù hợp.**

💡 **Gợi ý:**
- Thử tìm với từ khóa khác
- Mở rộng phạm vi tìm kiếm
- Tạo yêu cầu mua để người bán liên hệ với bạn

Bạn có muốn tạo yêu cầu tìm kiếm không?
`;
            return response;

        } catch (error) {
            console.error("🔥 [K2 Tool] searchMarketplace ERROR:", error);
            return `❌ Lỗi tìm kiếm: ${error instanceof Error ? error.message : "Unknown error"}`;
        }
    },

    inputSchema: z.object({
        query: z.string().describe("Từ khóa tìm kiếm"),
        topic: z.string().optional().describe("Lọc theo danh mục")
    }),

    outputSchema: z.string().describe("Kết quả tìm kiếm")
};

/**
 * Tool: Prepare Dynamic Form
 * 
 * Chuẩn bị form động để tạo yêu cầu giao dịch
 * Dispatch event để Marketplace hiển thị form và trả về text xác nhận
 */
const prepareDynamicFormTool = {
    name: "prepare-dynamic-form",
    description: `Chuẩn bị form động để tạo yêu cầu mua/bán/trao đổi.

QUAN TRỌNG - QUY TẮC SỬ DỤNG:
1. CHỈ gọi tool này SAU KHI đã gọi extract-marketplace-intent VÀ người dùng xác nhận
2. PHẢI sử dụng CHÍNH XÁC dữ liệu từ kết quả extract-marketplace-intent

CÁC GIÁ TRỊ HỢP LỆ:
- topic: "Digital Assets" | "Goods" | "Freelance Job"
- action: "buy" | "sell" | "exchange"

Nếu topic = "Digital Assets" hoặc "Goods":
- subtopic: "Video" | "Images" | "Audio" | "Token" | "License | Key | Secret" | "Document" | "Source Code" | "Dataset" | "Fashion" | "Electronics & Devices" | "Books & Learning" | "Sports & Travel"

Nếu topic = "Freelance Job":
- category: "Tech & IT" | "Design & Creative" | "Writing & Translation" | "Marketing & Sales"
- skill: "Web & Mobile Development" | "Software / App Development" | "Data Science / Analytics" | "IT Support / Networking" | "Graphic Design" | "UI/UX Design" | "Illustration / Animation" | "Video & Photo Editing" | "Content Writing / Copywriting" | "Blogging / Articles" | "Translation / Localization" | "Technical Writing" | "Digital Marketing" | "Social Media Management" | "SEO / SEM" | "Sales & Lead Generation"

VÍ DỤ:
- Intent: topic="Digital Assets", selection.subtopic="Video", action="buy"
- Gọi tool: topic="Digital Assets", subtopic="Video", action="buy"
- Intent: topic="Freelance Job", selection.category="Design & Creative", selection.skill="UI/UX Design", action="sell"
- Gọi tool: topic="Freelance Job", category="Design & Creative", skill="UI/UX Design", action="sell"`,

    tool: async (input: {
        topic: string;
        action: string;
        subtopic?: string;
        category?: string;
        skill?: string;
        title?: string;
        description?: string;
    }): Promise<string> => {
        console.log("🚀 [K2 Tool] prepareDynamicForm START", { input });

        try {
            const actionLabels: Record<string, string> = {
                buy: "MUA",
                sell: "BÁN",
                exchange: "TRAO ĐỔI"
            };

            // Build selection info
            const selectionInfo = input.topic === "Freelance Job"
                ? `${input.category || "N/A"} > ${input.skill || "N/A"}`
                : input.subtopic || "N/A";

            // Build form data
            const formData = {
                topic: input.topic,
                action: input.action,
                selection: input.topic === "Freelance Job"
                    ? { category: input.category || "", skill: input.skill || "" }
                    : { subtopic: input.subtopic || "" },
                title: input.title,
                description: input.description,
            };

            // Dispatch event to Marketplace to show form
            if (typeof window !== 'undefined') {
                window.dispatchEvent(new CustomEvent('k2:showDynamicForm', {
                    detail: { data: formData, streaming: false }
                }));

                // Dispatch event to show "Bắt đầu giao dịch" button in chat
                window.dispatchEvent(new CustomEvent('k2:showStartButton', {
                    detail: {
                        formData,
                        actionText: actionLabels[input.action] || input.action,
                        title: input.title || input.topic
                    }
                }));
            }

            const response = `**Đã tạo form yêu cầu giao dịch!**

**Hành động:** ${actionLabels[input.action] || input.action}
**Danh mục:** ${input.topic} > ${selectionInfo}
${input.title ? `**Tiêu đề:** ${input.title}` : ""}
${input.description ? `**Mô tả:** ${input.description}` : ""}

Vui lòng kiểm tra tab **Create Request** bên trái để xem form.
Khi sẵn sàng, nhấn nút **Bắt đầu giao dịch** bên dưới để tìm người phù hợp.`;

            return response;

        } catch (error) {
            console.error("🔥 [K2 Tool] prepareDynamicForm ERROR:", error);
            return `❌ Không thể tạo form: ${error instanceof Error ? error.message : "Unknown error"}`;
        }
    },

    inputSchema: z.object({
        topic: z.string().describe("Danh mục: Digital Assets, Goods, hoặc Freelance Job"),
        action: z.string().describe("Hành động: buy, sell, exchange"),
        subtopic: z.string().optional().describe("Danh mục con cho Digital Assets hoặc Goods"),
        category: z.string().optional().describe("Category cho Freelance Job"),
        skill: z.string().optional().describe("Skill cho Freelance Job"),
        title: z.string().optional().describe("Tiêu đề yêu cầu"),
        description: z.string().optional().describe("Mô tả chi tiết"),
    }),

    outputSchema: z.string().describe("Kết quả tạo form")
};

// Export all tools
export const tamboTools = [
    extractMarketplaceIntentTool,
    createTradeRequestTool,
    searchMarketplaceTool,
    prepareDynamicFormTool,
];
