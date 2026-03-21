use axum::{http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct ClassifyIntentBody {
    pub user_prompt: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

/// POST /api/classify-intent — proxy to Groq API for intent classification
pub async fn classify_intent(
    Json(body): Json<ClassifyIntentBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let base_url = body.base_url.unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
    let model = body.model.unwrap_or_else(|| "llama-3.3-70b-versatile".to_string());

    let system_prompt = r#"Bạn là classifier phân tích ý định người dùng trên K2 Marketplace.

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

Subtopic hợp lệ:
- Digital Assets: Video, Images, Audio, Token, License | Key | Secret, Document, Source Code, Dataset
- Goods: Fashion, Electronics & Devices, Books & Learning, Sports & Travel
- Freelance Job category: Tech & IT, Design & Creative, Writing & Translation, Marketing & Sales

Quy tắc:
- action="none" nếu chỉ hỏi thông tin
- needs_search=true CHỈ KHI có từ "tìm", "xem", "có ai", "danh sách", "list"
- needs_search=false khi muốn MUA/BÁN/TRAO ĐỔI (tạo yêu cầu giao dịch mới)
- title: tóm tắt ngắn gọn
- null cho field không xác định được"#;

    let request_body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": body.user_prompt}
        ],
        "response_format": {"type": "json_object"}
    });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", body.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err((StatusCode::BAD_GATEWAY, format!("Groq API error: {} - {}", status, error_text)));
    }

    let data: serde_json::Value = response.json().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON parse error: {}", e)))?;

    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "No content in response".to_string()))?;

    let result: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Content parse error: {}", e)))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct GroqChatBody {
    pub messages: serde_json::Value,
    pub tools: Option<serde_json::Value>,
    pub api_key: String,
    pub model: Option<String>,
}

/// POST /api/groq-chat — proxy Groq chat with tool_calls support
pub async fn groq_chat_with_tools(
    Json(body): Json<GroqChatBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let base_url = std::env::var("GROQ_BASE_URL")
        .unwrap_or_else(|_| "https://api.groq.com/openai/v1".to_string());
    let model = body.model.unwrap_or_else(|| "llama-3.3-70b-versatile".to_string());

    let mut request_body = json!({
        "model": model,
        "messages": body.messages,
        "temperature": 0.7
    });

    if let Some(tools) = body.tools {
        request_body["tools"] = tools;
        request_body["tool_choice"] = json!("auto");
    }

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", body.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err((StatusCode::BAD_GATEWAY, format!("Groq API error {}: {}", status, text)));
    }

    let data: serde_json::Value = response.json().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let message = &data["choices"][0]["message"];
    Ok(Json(json!({
        "content": message["content"].as_str().unwrap_or(""),
        "tool_calls": message.get("tool_calls").cloned().unwrap_or(serde_json::Value::Null)
    })))
}

#[derive(Deserialize)]
pub struct ClassifyK2Body {
    pub user_prompt: String,
}

/// POST /api/k2-endpoint — call K2 classification endpoint
pub async fn classify_k2_endpoint(
    Json(body): Json<ClassifyK2Body>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let endpoint = std::env::var("K2_ENDPOINT").unwrap_or_default();
    if endpoint.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "K2_ENDPOINT not configured".to_string()));
    }
    if !endpoint.starts_with("https://") {
        return Err((StatusCode::BAD_REQUEST, "K2_ENDPOINT must use HTTPS".to_string()));
    }

    let url = format!("{}/post?user_input={}", endpoint.trim_end_matches('/'), urlencoding::encode(&body.user_prompt));

    let client = reqwest::Client::new();
    let response = client.post(&url).send().await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err((StatusCode::BAD_GATEWAY, format!("K2 Endpoint error: {}", response.status())));
    }

    let text = response.text().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let result: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON parse error: {}", e)))?;

    Ok(Json(result))
}
