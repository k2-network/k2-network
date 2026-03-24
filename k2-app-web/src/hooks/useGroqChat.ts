import { useState, useCallback, useEffect } from 'react';
import { classifyIntent as apiClassifyIntent, groqChatWithTools, startListening, listenOffers } from '../api';
import { getChatHistory, saveChatMessages } from '../api/chat';

export interface Message {
    id: string;
    role: 'user' | 'assistant';
    content: string;
}

// ── Schema & Validation (Pydantic-style in TS) ────────────────────────────────

const VALID_TOPICS = ["Digital Assets", "Goods", "Freelance Job"] as const;
const VALID_ACTIONS = ["buy", "sell", "exchange", "none"] as const;

// Level 1: topic → subtopic (category trong Marketplace)
const VALID_SUBTOPICS: Record<string, string[]> = {
    "Digital Assets": ["Video", "Images", "Audio", "Token", "License | Key | Secret", "Document", "Source Code", "Dataset"],
    "Goods": ["Fashion", "Electronics & Devices", "Books & Learning", "Sports & Travel", "Toys & Games", "Home & Living"],
};

// Level 2: subtopic → sub-categories (hiển thị trong CategoryDetailView)
const VALID_SUB_CATEGORIES: Record<string, string[]> = {
    // Digital Assets
    "Video": ["Short Clips", "Full Movies", "Tutorials", "Stock Footage", "Animations", "Live Streams"],
    "Images": ["Photography", "Illustrations", "Vector Art", "Icons & UI Kits", "Wallpapers", "NFT Artwork"],
    "Audio": ["Music Tracks", "Sound Effects", "Podcasts", "Voice Overs", "Samples & Loops", "ASMR"],
    "Token": ["ERC-20 Tokens", "NFTs", "Game Tokens", "Utility Tokens", "Governance Tokens", "Stablecoins"],
    "License | Key | Secret": ["Software Licenses", "API Keys", "Game Keys", "Subscription Access", "Domain Access", "SSL Certificates"],
    "Document": ["Templates", "Research Papers", "E-Books", "Legal Documents", "Business Plans", "Whitepapers"],
    "Source Code": ["Full Projects", "Scripts & Snippets", "Libraries", "Plugins", "Themes", "Bots & Automation"],
    "Dataset": ["Training Data", "Financial Data", "Market Research", "User Behavior", "Geospatial Data", "Medical Records"],
    // Goods
    "Fashion": ["Clothing", "Shoes & Footwear", "Accessories", "Bags & Luggage", "Jewelry", "Vintage & Luxury"],
    "Electronics & Devices": ["Smartphones", "Laptops & PCs", "Cameras", "Audio Equipment", "Gaming Gear", "Smart Home"],
    "Books & Learning": ["Fiction", "Non-Fiction", "Textbooks", "Magazines", "Comics & Manga", "Study Materials"],
    "Sports & Travel": ["Sports Equipment", "Outdoor Gear", "Travel Accessories", "Fitness", "Cycling", "Water Sports"],
    "Toys & Games": ["Rubik's Cube & Speed Cubes", "Action Figures & Collectibles", "Board Games & Card Games", "LEGO & Building Blocks", "Remote Control Toys", "Puzzles", "Stuffed Animals & Plushies", "Educational Toys", "Diecast & Model Cars", "Trading Card Games (TCG)", "Anime & Manga Figures"],
    "Home & Living": ["Kitchen & Cooking", "Furniture & Decor", "Bedding & Pillows", "Bathroom Essentials", "Cleaning & Organizers", "Lighting", "Plants & Gardening", "Air Purifiers & Fans", "Rice Cookers & Small Appliances", "Storage & Shelving", "Wall Art & Frames", "Candles & Aromatherapy"],
};

const VALID_CATEGORIES = ["Tech & IT", "Design & Creative", "Writing & Translation", "Marketing & Sales"];
const FREELANCE_SKILLS: Record<string, string[]> = {
    "Tech & IT": ["Web & Mobile Development", "Software / App Development", "Data Science / Analytics", "IT Support / Networking"],
    "Design & Creative": ["Graphic Design", "UI/UX Design", "Illustration / Animation", "Video & Photo Editing"],
    "Writing & Translation": ["Content Writing / Copywriting", "Blogging / Articles", "Translation / Localization", "Technical Writing"],
    "Marketing & Sales": ["Digital Marketing", "Social Media Management", "SEO / SEM", "Sales & Lead Generation"],
};

type Topic = typeof VALID_TOPICS[number];
type Action = typeof VALID_ACTIONS[number];

interface IntentResult {
    action: Action;
    topic: Topic | null;
    subtopic?: string;      // level 1: e.g. "Electronics & Devices"
    sub_category?: string;  // level 2: e.g. "Smartphones"
    category?: string;      // Freelance Job category
    skill?: string;
    title: string;
    description: string;
    needs_search: boolean;
}

/**
 * Validate và coerce JSON từ Groq về đúng schema.
 * Throw nếu không thể fix được.
 */
function fuzzyMatch(value: string, options: string[]): string | undefined {
    const lower = value.toLowerCase();
    return options.find(o =>
        o.toLowerCase() === lower ||
        o.toLowerCase().includes(lower) ||
        lower.includes(o.toLowerCase().split(' ')[0])
    );
}

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

    // subtopic (level 1) - coerce nếu không hợp lệ
    let subtopic = raw.subtopic || raw.selection?.subtopic;
    if (topic && topic !== "Freelance Job" && subtopic) {
        const validSubs = VALID_SUBTOPICS[topic] || [];
        if (!validSubs.includes(subtopic)) {
            subtopic = fuzzyMatch(subtopic, validSubs) ?? undefined;
        }
    }

    // sub_category (level 2) — AI có thể trả về trực tiếp
    let sub_category = raw.sub_category || raw.selection?.sub_category;
    if (subtopic && sub_category) {
        const validSubCats = VALID_SUB_CATEGORIES[subtopic] || [];
        if (!validSubCats.includes(sub_category)) {
            sub_category = fuzzyMatch(sub_category, validSubCats) ?? undefined;
        }
    }
    // Nếu AI trả về sub_category nhưng chưa có subtopic, thử suy ngược subtopic
    if (!subtopic && sub_category && topic && topic !== "Freelance Job") {
        const validSubs = VALID_SUBTOPICS[topic] || [];
        for (const sub of validSubs) {
            const subCats = VALID_SUB_CATEGORIES[sub] || [];
            if (subCats.includes(sub_category) || fuzzyMatch(sub_category, subCats)) {
                subtopic = sub;
                sub_category = fuzzyMatch(sub_category, subCats) ?? sub_category;
                break;
            }
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
        sub_category,
        category,
        skill,
        title: raw.title || raw.description || "",
        description: raw.description || "",
        needs_search: raw.needs_search === true,
    };
}

// ── Intent classification với JSON mode + retry ───────────────────────────────


async function classifyIntent(userPrompt: string, sessionId: string, model: string, retries = 2): Promise<IntentResult | null> {
    for (let attempt = 0; attempt <= retries; attempt++) {
        try {
            const raw = await apiClassifyIntent({
                user_prompt: userPrompt,
                session_id: sessionId || undefined,
                base_url: import.meta.env.VITE_GROQ_BASE_URL || 'https://api.groq.com/openai/v1',
                model: import.meta.env.VITE_GROQ_SMALL_MODEL || model,
            });
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
            : { subtopic: intent.subtopic || "", sub_category: intent.sub_category || undefined },
        title: intent.title,
        description: intent.description,
    };

    window.dispatchEvent(new CustomEvent('k2:showDynamicForm', {
        detail: { data: formData, streaming: false }
    }));
    window.dispatchEvent(new CustomEvent('k2:intentChanged', {
        detail: { action: intent.action, topic: intent.topic, subtopic: intent.subtopic, sub_category: intent.sub_category }
    }));
    window.dispatchEvent(new CustomEvent('k2:showStartButton', {
        detail: {
            actionText: actionLabels[intent.action] || intent.action,
            title: intent.title || intent.topic
        }
    }));

    const selectionInfo = intent.topic === "Freelance Job"
        ? `${intent.category} > ${intent.skill}`
        : intent.sub_category
            ? `${intent.subtopic} > ${intent.sub_category}`
            : intent.subtopic || "N/A";
    return `Da tao form ${actionLabels[intent.action]} - ${intent.topic} > ${selectionInfo}`;
}

async function executeSearch(intent: IntentResult): Promise<string> {
    try {
        const topics = intent.topic ? [intent.topic] : ["Digital Assets", "Goods", "Freelance Job"];
        const allOffers: any[] = [];
        for (const topic of topics) {
            try {
                await startListening(topic);
                const offers = await listenOffers(topic, 5) as any[];
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

// ── Clarification logic ───────────────────────────────────────────────────────

interface ClarificationNeeded {
    missing: 'topic' | 'subtopic' | 'sub_category' | 'category' | 'skill';
    question: string;
    options: string[];
}

/**
 * Kiểm tra intent có đủ thông tin để tạo form không.
 * Trả về null nếu đủ, hoặc ClarificationNeeded nếu cần hỏi thêm.
 * Thứ tự hỏi: topic → subtopic → sub_category → (freelance: category → skill)
 */
function checkNeedsClarification(intent: IntentResult): ClarificationNeeded | null {
    if (!intent.action || intent.action === "none") return null;

    // 1. Thiếu topic
    if (!intent.topic) {
        return {
            missing: 'topic',
            question: "Bạn muốn giao dịch trong lĩnh vực nào?",
            options: ["Digital Assets", "Goods", "Freelance Job"],
        };
    }

    if (intent.topic !== "Freelance Job") {
        const validSubs = VALID_SUBTOPICS[intent.topic] || [];

        // 2. Thiếu subtopic (level 1)
        if (!intent.subtopic || !validSubs.includes(intent.subtopic)) {
            return {
                missing: 'subtopic',
                question: `Trong **${intent.topic}**, bạn quan tâm đến danh mục nào?`,
                options: validSubs,
            };
        }

        // 3. Thiếu sub_category (level 2)
        const validSubCats = VALID_SUB_CATEGORIES[intent.subtopic] || [];
        if (validSubCats.length > 0 && (!intent.sub_category || !validSubCats.includes(intent.sub_category))) {
            return {
                missing: 'sub_category',
                question: `Trong **${intent.subtopic}**, bạn cần loại sản phẩm nào cụ thể?`,
                options: validSubCats,
            };
        }
    }

    // Freelance Job
    if (intent.topic === "Freelance Job") {
        if (!intent.category || !VALID_CATEGORIES.includes(intent.category)) {
            return {
                missing: 'category',
                question: "Bạn cần dịch vụ freelance thuộc lĩnh vực nào?",
                options: VALID_CATEGORIES,
            };
        }
        const validSkills = FREELANCE_SKILLS[intent.category] || [];
        if (!intent.skill || !validSkills.includes(intent.skill)) {
            return {
                missing: 'skill',
                question: `Trong **${intent.category}**, bạn cần kỹ năng cụ thể nào?`,
                options: validSkills,
            };
        }
    }

    return null;
}

/**
 * Tạo message hỏi clarification cho user.
 */
function buildClarificationMessage(c: ClarificationNeeded): string {
    const optionLines = c.options.map((o, i) => `${i + 1}. ${o}`).join('\n');
    return `${c.question}\n\n${optionLines}\n\nBạn có thể trả lời bằng số hoặc tên danh mục.`;
}

// ── Pending intent store (để merge clarification answer) ─────────────────────
// Lưu intent đang chờ clarification
let pendingIntent: IntentResult | null = null;

/**
 * Thử merge câu trả lời clarification vào pendingIntent.
 * Trả về intent đã merge nếu thành công, null nếu không có pending.
 */
function tryMergeClarification(userInput: string): IntentResult | null {
    if (!pendingIntent) return null;

    const lower = userInput.toLowerCase().trim();

    // Check xem đang thiếu field gì
    const missing = checkNeedsClarification(pendingIntent);
    if (!missing) return pendingIntent; // đã đủ

    const { options } = missing;

    // 1. Match theo số (1, 2, 3...)
    const numMatch = lower.match(/^(\d+)$/);
    if (numMatch) {
        const idx = parseInt(numMatch[1]) - 1;
        if (idx >= 0 && idx < options.length) {
            return applyOption(pendingIntent, missing.missing, options[idx]);
        }
    }

    // 2. Exact match (case-insensitive)
    const exactMatch = options.find(o => o.toLowerCase() === lower);
    if (exactMatch) {
        return applyOption(pendingIntent, missing.missing, exactMatch);
    }

    // 3. Option chứa toàn bộ input (e.g. "short" → "Short Clips")
    const containsInput = options.find(o => o.toLowerCase().includes(lower));
    if (containsInput) {
        return applyOption(pendingIntent, missing.missing, containsInput);
    }

    // 4. Input chứa toàn bộ option (e.g. "short clips videos" → "Short Clips")
    const inputContainsOption = options.find(o => lower.includes(o.toLowerCase()));
    if (inputContainsOption) {
        return applyOption(pendingIntent, missing.missing, inputContainsOption);
    }

    return null; // không nhận ra câu trả lời
}

function applyOption(intent: IntentResult, field: ClarificationNeeded['missing'], value: string): IntentResult {
    const updated = { ...intent };
    if (field === 'topic') updated.topic = value as IntentResult['topic'];
    if (field === 'subtopic') { updated.subtopic = value; updated.sub_category = undefined; } // reset sub_category khi đổi subtopic
    if (field === 'sub_category') updated.sub_category = value;
    if (field === 'category') { updated.category = value; updated.skill = undefined; }
    if (field === 'skill') updated.skill = value;
    pendingIntent = updated;
    return updated;
}

// ── System prompts ────────────────────────────────────────────────────────────
// Note: intent classification system prompt is managed by the backend (ai.rs)

const CHAT_SYSTEM_PROMPT = `Bạn là K2 Assistant - trợ lý AI thông minh tích hợp trong K2 Marketplace P2P phi tập trung.

Bạn có thể:
- Trả lời mọi câu hỏi thông thường (kiến thức, lập trình, tư vấn, v.v.)
- Hỗ trợ người dùng giao dịch trên K2 Marketplace (mua, bán, trao đổi hàng hóa, tài sản số, freelance)
- Giải thích kết quả sau khi thực hiện các action trên marketplace
- Hướng dẫn người dùng sử dụng K2 Platform

Phong cách: ngắn gọn, thân thiện, trả lời bằng tiếng Việt. Nếu câu hỏi không liên quan marketplace thì trả lời bình thường như một AI assistant.`;

// Keywords cho marketplace intent — chỉ gọi classifyIntent khi có những từ này
const MARKETPLACE_KEYWORDS = [
    'mua', 'bán', 'trao đổi', 'exchange', 'buy', 'sell', 'trade',
    'giao dịch', 'thuê', 'cần thuê', 'offer', 'freelance',
    'tìm người', 'tìm mua', 'tìm bán', 'cần mua', 'cần bán',
    'muốn mua', 'muốn bán', 'muốn trao', 'đang bán', 'đang mua',
];

function looksLikeMarketplaceIntent(text: string): boolean {
    const lower = text.toLowerCase();
    return MARKETPLACE_KEYWORDS.some(kw => lower.includes(kw));
}

// ── Hook ──────────────────────────────────────────────────────────────────────

export const useGroqChat = (sessionId?: string) => {
    const [messages, setMessages] = useState<Message[]>([]);
    const [isProcessing, setIsProcessing] = useState(false);
    // Start as true if sessionId is provided — avoids welcome-screen flash before history loads
    const [isLoadingHistory, setIsLoadingHistory] = useState(!!sessionId);

    // Load chat history from backend when sessionId is set
    useEffect(() => {
        if (!sessionId) return;
        setIsLoadingHistory(true);
        getChatHistory(sessionId, 'ai', 50)
            .then(({ messages: history }) => {
                if (history.length > 0) {
                    setMessages(history.map(h => ({
                        id: h.id,
                        role: h.role as 'user' | 'assistant',
                        content: h.content,
                    })));
                }
            })
            .catch(() => { /* ignore — start fresh if history unavailable */ })
            .finally(() => setIsLoadingHistory(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [sessionId]);

    const sendMessage = useCallback(async (content: string) => {
        if (!content.trim()) return;
        const model = import.meta.env.VITE_GROQ_MODEL || 'llama-3.3-70b-versatile';

        const userMsg: Message = { id: Date.now().toString(), role: 'user', content };
        const updatedMessages = [...messages, userMsg];
        setMessages(updatedMessages);
        setIsProcessing(true);

        try {
            let intent: IntentResult | null = null;
            let assistantContent = "";

            // Bước 1: Thử merge nếu đang có pending clarification
            const mergedIntent = tryMergeClarification(content);
            if (mergedIntent) {
                intent = mergedIntent;
                console.log("[Intent] Merged clarification →", intent);
            } else if (pendingIntent) {
                // Có pendingIntent nhưng không nhận ra câu trả lời → hỏi lại, không classify fresh
                console.log("[Intent] Pending exists but answer unrecognized, re-asking clarification");
                const clarification = checkNeedsClarification(pendingIntent);
                if (clarification) {
                    assistantContent = `Mình chưa nhận ra câu trả lời đó. ${buildClarificationMessage(clarification)}`;
                } else {
                    intent = pendingIntent;
                }
            } else if (looksLikeMarketplaceIntent(content)) {
                // Chỉ classify khi có từ khóa mua/bán — tránh misclassify câu hỏi thường
                intent = await classifyIntent(content, sessionId || '', model);
                console.log("[Intent] Fresh classify →", intent);
                if (intent && intent.action !== "none") {
                    intent = { ...intent, sub_category: undefined, skill: undefined };
                    pendingIntent = intent;
                }
            }
            // else: không có keyword → intent = null → Groq trả lời tự nhiên

            let toolResultContext = "";

            // Bước 2: Nếu có action giao dịch → check clarification
            if (intent && intent.action !== "none") {
                const clarification = checkNeedsClarification(intent);

                if (clarification) {
                    console.log("[Clarification needed]", clarification);
                    assistantContent = buildClarificationMessage(clarification);
                } else {
                    pendingIntent = null;
                    if (intent.needs_search) {
                        toolResultContext = await executeSearch(intent);
                    } else {
                        toolResultContext = await executePrepareForm(intent);
                    }
                }
            }

            // Bước 3: Nếu đã có clarification message → dùng luôn, không gọi Groq
            if (assistantContent) {
                setMessages(prev => [...prev, {
                    id: (Date.now() + 1).toString(),
                    role: 'assistant',
                    content: assistantContent,
                }]);
                // Save clarification exchange to backend
                if (sessionId) {
                    saveChatMessages(sessionId, 'ai', [
                        { role: 'user', content },
                        { role: 'assistant', content: assistantContent },
                    ]).catch(() => {});
                }
                return;
            }

            // Bước 4: Groq trả lời tự nhiên dựa trên context
            const chatMessages = [
                { role: 'system', content: CHAT_SYSTEM_PROMPT },
                ...updatedMessages.map(m => ({ role: m.role, content: m.content })),
                ...(toolResultContext ? [{
                    role: 'system' as const,
                    content: `[Kết quả action]: ${toolResultContext}`
                }] : []),
            ];

            const response = await groqChatWithTools({
                messages: chatMessages,
                tools: undefined,
                session_id: sessionId || undefined,
                model,
            });

            const assistantMsg: Message = {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content: response.content || '',
            };
            setMessages(prev => [...prev, assistantMsg]);

            // Save Groq exchange to backend
            if (sessionId && assistantMsg.content) {
                saveChatMessages(sessionId, 'ai', [
                    { role: 'user', content },
                    { role: 'assistant', content: assistantMsg.content },
                ]).catch(() => {});
            }

        } catch (error) {
            setMessages(prev => [...prev, {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content: `Xin loi, co loi: ${error instanceof Error ? error.message : String(error)}`,
            }]);
        } finally {
            setIsProcessing(false);
        }
    }, [sessionId, messages]);

    const resetChat = () => { setMessages([]); pendingIntent = null; };

    const deleteMessage = useCallback((id: string) => {
        setMessages(prev => prev.filter(m => m.id !== id));
    }, []);

    return { messages, sendMessage, isProcessing, isLoadingHistory, resetChat, deleteMessage };
};
