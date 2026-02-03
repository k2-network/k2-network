/**
 * Tambo Tools for K2 Marketplace
 * 
 * Tools để Tambo AI có thể gọi và thực hiện các action
 */
import { z } from "zod";
import { extractMarketplaceIntent } from "../services/groqStructuredOutput";

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
            const result = await extractMarketplaceIntent(input.userPrompt);
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

// Export all tools
export const tamboTools = [
    extractMarketplaceIntentTool,
    createTradeRequestTool,
    searchMarketplaceTool,
];
