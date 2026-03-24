/**
 * Groq Structured Output Service
 * 
 * Sử dụng Groq API với strict mode để trả về JSON theo schema
 * Tham khảo: https://console.groq.com/docs/structured-outputs
 */

// Schema cho marketplace selection
export const marketplaceSelectionSchema = {
    type: "object",
    properties: {
        topic: {
            type: "string",
            enum: ["Digital Assets", "Goods", "Freelance Job"]
        },
        selection: {
            anyOf: [
                {
                    type: "object",
                    properties: {
                        subtopic: {
                            type: "string",
                            enum: [
                                "Video", "Images", "Audio", "Token",
                                "License | Key | Secret", "Document",
                                "Source Code", "Dataset", "Fashion",
                                "Electronics & Devices", "Books & Learning", "Sports & Travel"
                            ]
                        }
                    },
                    required: ["subtopic"],
                    additionalProperties: false
                },
                {
                    type: "object",
                    properties: {
                        category: {
                            type: "string",
                            enum: ["Tech & IT", "Design & Creative", "Writing & Translation", "Marketing & Sales"]
                        },
                        skill: {
                            type: "string",
                            enum: [
                                "Web & Mobile Development", "Software / App Development", "Data Science / Analytics", "IT Support / Networking",
                                "Graphic Design", "UI/UX Design", "Illustration / Animation", "Video & Photo Editing",
                                "Content Writing / Copywriting", "Blogging / Articles", "Translation / Localization", "Technical Writing",
                                "Digital Marketing", "Social Media Management", "SEO / SEM", "Sales & Lead Generation"
                            ]
                        }
                    },
                    required: ["category", "skill"],
                    additionalProperties: false
                }
            ]
        },
        action: {
            type: "string",
            enum: ["buy", "sell", "exchange"],
            description: "Hành động người dùng muốn thực hiện"
        },
        description: {
            type: ["string", "null"],
            description: "Mô tả chi tiết yêu cầu của người dùng"
        }
    },
    required: ["topic", "selection", "action", "description"],
    additionalProperties: false
};

export interface MarketplaceSelection {
    topic: "Digital Assets" | "Goods" | "Freelance Job";
    selection:
    | { subtopic: string }
    | { category: string; skill: string };
    action: "buy" | "sell" | "exchange";
    description: string | null;
}

/**
 * Gọi Groq API với structured output (strict mode)
 */
export async function extractMarketplaceIntent(userPrompt: string): Promise<MarketplaceSelection> {
    const apiKey = import.meta.env.VITE_GROQ_API_KEY;
    const baseUrl = import.meta.env.VITE_GROQ_BASE_URL || 'https://api.groq.com/openai/v1';
    const model = import.meta.env.VITE_GROQ_SMALL_MODEL || 'openai/gpt-oss-20b';

    if (!apiKey) {
        throw new Error('VITE_GROQ_API_KEY is not configured');
    }

    const response = await fetch(`${baseUrl}/chat/completions`, {
        method: 'POST',
        headers: {
            'Authorization': `Bearer ${apiKey}`,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({
            model: model,
            messages: [
                {
                    role: "system",
                    content: `Bạn là AI phân tích yêu cầu mua bán trên K2 Marketplace. Phân tích ý định của người dùng và trích xuất thông tin theo schema.

Các topic:
- "Digital Assets": Video, Images, Audio, Token, License | Key | Secret, Document, Source Code, Dataset
- "Goods": Fashion, Electronics & Devices, Books & Learning, Sports & Travel  
- "Freelance Job": Tech & IT, Design & Creative, Writing & Translation, Marketing & Sales (mỗi category có các skills cụ thể)

Các action:
- "buy": Người dùng muốn MUA
- "sell": Người dùng muốn BÁN
- "exchange": Người dùng muốn TRAO ĐỔI

Ví dụ:
- "Tôi muốn mua video" -> topic: "Digital Assets", selection: {subtopic: "Video"}, action: "buy"
- "Bán dịch vụ thiết kế UI/UX" -> topic: "Freelance Job", selection: {category: "Design & Creative", skill: "UI/UX Design"}, action: "sell"
- "Trao đổi laptop" -> topic: "Goods", selection: {subtopic: "Electronics & Devices"}, action: "exchange"`
                },
                {
                    role: "user",
                    content: userPrompt
                }
            ],
            response_format: {
                type: "json_schema",
                json_schema: {
                    name: "marketplace_selection",
                    strict: true,
                    schema: marketplaceSelectionSchema
                }
            }
        })
    });

    if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new Error(`Groq API Error: ${response.status} - ${JSON.stringify(errorData)}`);
    }

    const data = await response.json();
    const content = data.choices[0]?.message?.content;

    if (!content) {
        throw new Error('No content received from Groq API');
    }

    return JSON.parse(content) as MarketplaceSelection;
}
