import { useState, useCallback } from 'react';

export interface Message {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
}

export const useGroqChat = () => {
    const [messages, setMessages] = useState<Message[]>([]);
    const [isProcessing, setIsProcessing] = useState(false);
    // Prioritize env var, then local storage, then empty
    const [apiKey, setApiKey] = useState<string>(() =>
        import.meta.env.VITE_GROQ_API_KEY || localStorage.getItem('GROQ_API_KEY') || ''
    );

    const saveApiKey = (key: string) => {
        setApiKey(key);
        localStorage.setItem('GROQ_API_KEY', key);
    };

    const sendMessage = useCallback(async (content: string) => {
        if (!content.trim()) return;

        // Final check for API key (though env var should handle it)
        const effectiveApiKey = apiKey || import.meta.env.VITE_GROQ_API_KEY;

        if (!effectiveApiKey) {
            alert('Please configure your Groq API Key!');
            return;
        }

        const newMessage: Message = {
            id: Date.now().toString(),
            role: 'user',
            content
        };

        setMessages(prev => [...prev, newMessage]);
        setIsProcessing(true);

        try {
            const baseUrl = import.meta.env.VITE_GROQ_BASE_URL || 'https://api.groq.com/openai/v1';
            const model = import.meta.env.VITE_GROQ_MODEL || 'llama-3.3-70b-versatile';

            const response = await fetch(`${baseUrl}/chat/completions`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${effectiveApiKey}`,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    model: model,
                    messages: [
                        {
                            role: "system",
                            content: `Bạn là AI Assistant của K2 Marketplace. Bạn giúp người dùng tìm kiếm sản phẩm, tạo yêu cầu mua bán, và đàm phán P2P. Hãy trả lời ngắn gọn, thân thiện bằng tiếng Việt, không sử dụng Icon.`
                        },
                        ...messages.map(m => ({ role: m.role, content: m.content })),
                        { role: "user", content }
                    ],
                    temperature: 0.7
                })
            });

            if (!response.ok) {
                const errorData = await response.json().catch(() => ({}));
                throw new Error(`Groq API Error: ${response.status} - ${JSON.stringify(errorData)}`);
            }

            const data = await response.json();
            const assistantMessage: Message = {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content: data.choices[0]?.message?.content || "No content received"
            };

            setMessages(prev => [...prev, assistantMessage]);
        } catch (error) {
            console.error(error);
            const errorMessage: Message = {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content: `Xin lỗi, có lỗi xảy ra: ${error instanceof Error ? error.message : String(error)}`
            };
            setMessages(prev => [...prev, errorMessage]);
        } finally {
            setIsProcessing(false);
        }
    }, [apiKey, messages]);

    const resetChat = () => {
        setMessages([]);
    };

    return {
        messages,
        sendMessage,
        isProcessing,
        apiKey,
        setApiKey: saveApiKey,
        resetChat
    };
};
