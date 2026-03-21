import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface Message {
    id: string;
    role: 'user' | 'assistant';
    content: string;
}

// ── Schema & Validation (Pydantic-style in TS) ────────────────────────────────

const VALID_TOPICS = ["Digital Assets", "Goods", "Freelance Job"] as const;
const VALID_ACTIONS = ["buy", "sell", "exchange", "none"] as const;
const VALID_SUBTOPICS: Record<string, string[]> = {
    "Digital Assets": ["Video", "Images", "Audio", "Token", "License | Key | Secret", "Document", "Source Code", "Dataset"],
    "Goods": ["Fashion", "Electronics & Devices", "Books & Learning", "Sports & Travel"],
};
const VALID_CATEGORIES = ["Tech & IT", "Design & Creative", "Writing & Translation", "Marketing & Sales"];

type Topic = typeof VALID_TOPICS[number];
type Action = typeof VALID_ACTIONS[number];

interface IntentResult {
    action: Action;
    topic: Topic | null;
    subtopic?: string;
    category?: string;
    skill?: string;
    title: string;
    description: string;
    needs_search: boolean;
}

/**
 * Validate và coerce JSON từ Groq về đúng schema.
 * Throw nếu không thể fix được.
 */
function validateIntent(raw: any): IntentResult {
    // action
    const action = VALID_ACTIONS.includes(raw.action) ? raw.action : "none";

    // topic - fuzzy match
    let topic: Topic | null = null;
    if (raw.topic) {
        topic = VALID_TOPICS.find(t =>
            t.toLowerCase() === String(raw.topic).toLowerCase()
        ) ?? null;
    }

    // subtopic - coerce nếu không hợp lệ
    let subtopic = raw.subtopic || raw.selection?.subtopic;
    if (topic && topic !== "Freelance Job" && subtopic) {
        const validSubs = VALID_SUBTOPICS[topic] || [];
        if (!validSubs.includes(subtopic)) {
            // fuzzy match
            subtopic = validSubs.find(s =>
                s.toLowerCase().includes(subtopic.toLowerCase()) ||
                subtopic.toLowerCase().includes(s.toLowerCase())
            ) ?? validSubs[0];
        }
    }

    // category + skill cho Freelance Job
    let category = raw.category || raw.selection?.category;
    const skill = raw.skill || raw.selection?.skill;
    if (topic === "Freelance Job" && category) {
        category = VALID_CATEGORIES.find(c =>
            c.toLowerCase() === category.toLowerCase()
        ) ?? category;
    }

    return {
        action,
        topic,
        subtopic,
        category,
        skill,
        title: raw.title || raw.description || "",
        description: raw.description || "",
        needs_search: raw.needs_search === true,
    };
}

// ── Intent classification với JSON mode + retry ───────────────────────────────

const INTENT_SYSTEM_PROMPT = `Bạn là classifier phân tích ý định người dùng trên K2 Marketplace.

Trả về JSON CHÍNH XÁC theo schema sau, không thêm bất kỳ field nào khác:
{
  "action": "buy" | "sell" | "exchange" | "none",
  "topic": "Digital Assets" | "Goods" | "Freelance Job" | null,
  "subtopic": string | null,
  "category": string | null,
  "skill": string | null,
  "title": string,
  "description": string,
  "needs_search": boolean
}

Các subtopic hợp lệ:
- Digital Assets: Video, Images, Audio, Token, License | Key | Secret, Document, Source Code, Dataset
- Goods: Fashion, Electronics & Devices, Books & Learning, Sports & Travel
- Freelance Job category: Tech & IT, Design & Creative, Writing & Translation, Marketing & Sales

Quy tắc:
- action="none" nếu chỉ hỏi thông tin, không giao dịch
- needs_search=true nếu người dùng muốn TÌM KIẾM hoặc XEM có ai đang bán/mua
- needs_search=false nếu muốn TẠO yêu cầu mới
- title: tóm tắt ngắn gọn yêu cầu
- Trả về null cho field không xác định được`;

async function classifyIntent(userPrompt: string, apiKey: string, model: string, retries = 2): Promise<IntentResult | null> {
    for (let attempt = 0; attempt <= retries; attempt++) {
        try {
            const raw = await invoke<any>('classify_intent', {
                userPrompt,
                apiKey,
                baseUrl: import.meta.env.VITE_GROQ_BASE_URL || 'https://api.groq.com/openai/v1',
                model: import.meta.env.VITE_GROQ_SMALL_MODEL || model,
            });

            // Override system prompt bằng cách gọi thẳng groq_chat_with_tools với json mode
            // classify_intent đã dùng json_object mode, ta validate kết quả
            const validated = validateIntent(raw);
            return validated;
        } catch (e) {
            console.warn(`[Intent] Attempt ${attempt + 1} failed:`, e);
            if (attempt === retries) return null;
        }
    }
    return null;
}

// ── Tool executor ─────────────────────────────────────────────────────────────

async function executePrepareForm(intent: IntentResult): Promise<string> {
    const actionLabels: Record<string, string> = { buy: "MUA", sell: "BÁN", exchange: "TRAO ĐỔI" };
    const formData = {
        topic: intent.topic,
        action: intent.action,
        selection: intent.topic === "Freelance Job"
            ? { category: intent.category || "", skill: intent.skill || "" }
            : { subtopic: intent.subtopic || "" },
        title: intent.title,
        description: intent.description,
    };

    window.dispatchEvent(new CustomEvent('k2:showDynamicForm', {
        detail: { data: formData, streaming: false }
    }));
    window.dispatchEvent(new CustomEvent('k2:showStartButton', {
        detail: {
            actionText: actionLabels[intent.action] || intent.action,
            title: intent.title || intent.topic
        }
    }));

    const selectionInfo = intent.topic === "Freelance Job"
        ? `${intent.category} > ${intent.skill}`
        : intent.subtopic || "N/A";
    return `Da tao form ${actionLabels[intent.action]} - ${intent.topic} > ${selectionInfo}`;
}

async function executeSearch(intent: IntentResult): Promise<string> {
    try {
        const topics = intent.topic ? [intent.topic] : ["Digital Assets", "Goods", "Freelance Job"];
        const allOffers: any[] = [];
        for (const topic of topics) {
            try {
                await invoke('start_listening', { topic });
                const offers = await invoke<any[]>('listen_offers', { topic, timeoutSecs: 5 });
                allOffers.push(...offers);
            } catch { /* skip */ }
        }
        const q = (intent.subtopic || intent.skill || intent.description || "").toLowerCase();
        const matched = q
            ? allOffers.filter(o => {
                const fd = o.form_data || o;
                return (fd.title || "").toLowerCase().includes(q) || (fd.description || "").toLowerCase().includes(q);
            })
            : allOffers;

        if (matched.length === 0) return `Khong tim thay ket qua tren P2P luc nay.`;
        return matched.slice(0, 5).map((o, i) => {
            const fd = o.form_data || o;
            const price = fd.priceRange
                ? `${fd.priceRange.min?.toLocaleString()}-${fd.priceRange.max?.toLocaleString()} ${fd.priceRange.currency || 'VND'}`
                : "Thuong luong";
            return `${i + 1}. ${fd.title || "N/A"} | ${price} | node:${(o.sender_node_id || "").slice(0, 8)}...`;
        }).join("\n");
    } catch (e) {
        return `Loi tim kiem: ${e instanceof Error ? e.message : String(e)}`;
    }
}

// ── Chat system prompt ────────────────────────────────────────────────────────

const CHAT_SYSTEM_PROMPT = `Bạn là K2 Assistant - trợ lý AI cho K2 Marketplace P2P phi tập trung.

Nhiệm vụ:
- Giải thích kết quả sau khi thực hiện action (form đã tạo, kết quả tìm kiếm)
- Trả lời câu hỏi về K2 Marketplace
- Hướng dẫn người dùng bước tiếp theo

Phong cách: ngắn gọn, thân thiện, tiếng Việt.`;

// ── Hook ──────────────────────────────────────────────────────────────────────

export const useGroqChat = () => {
    const [messages, setMessages] = useState<Message[]>([]);
    const [isProcessing, setIsProcessing] = useState(false);
    const [apiKey, setApiKey] = useState<string>(
        import.meta.env.VITE_GROQ_API_KEY || ''
    );

    const sendMessage = useCallback(async (content: string) => {
        if (!content.trim()) return;

        const effectiveApiKey = apiKey || import.meta.env.VITE_GROQ_API_KEY;
        if (!effectiveApiKey) {
            alert('Vui long cau hinh Groq API Key!');
            return;
        }
        const model = import.meta.env.VITE_GROQ_MODEL || 'llama-3.3-70b-versatile';

        const userMsg: Message = { id: Date.now().toString(), role: 'user', content };
        const updatedMessages = [...messages, userMsg];
        setMessages(updatedMessages);
        setIsProcessing(true);

        try {
            // Bước 1: Classify intent với JSON mode + validation
            const intent = await classifyIntent(content, effectiveApiKey, model);
            console.log("[Intent]", intent);

            let toolResultContext = "";

            // Bước 2: Execute tool nếu có action hợp lệ
            if (intent && intent.action !== "none" && intent.topic) {
                if (intent.needs_search) {
                    toolResultContext = await executeSearch(intent);
                } else {
                    toolResultContext = await executePrepareForm(intent);
                }
            }

            // Bước 3: Groq trả lời tự nhiên dựa trên context
            const chatMessages = [
                { role: 'system', content: CHAT_SYSTEM_PROMPT },
                ...updatedMessages.map(m => ({ role: m.role, content: m.content })),
                ...(toolResultContext ? [{
                    role: 'system' as const,
                    content: `[Kết quả action]: ${toolResultContext}`
                }] : []),
            ];

            const response = await invoke<any>('groq_chat_with_tools', {
                messages: chatMessages,
                tools: null,
                apiKey: effectiveApiKey,
                model,
            });

            const assistantMsg: Message = {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content: response.content || '',
            };
            setMessages(prev => [...prev, assistantMsg]);

        } catch (error) {
            setMessages(prev => [...prev, {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content: `Xin loi, co loi: ${error instanceof Error ? error.message : String(error)}`,
            }]);
        } finally {
            setIsProcessing(false);
        }
    }, [apiKey, messages]);

    const resetChat = () => setMessages([]);

    return { messages, sendMessage, isProcessing, apiKey, setApiKey, resetChat };
};
