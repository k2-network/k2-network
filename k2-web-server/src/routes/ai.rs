use std::sync::Arc;
use axum::{extract::{State, Query}, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use crate::state::AppState;

// ── Helper: lấy Groq API key ──────────────────────────────────────────────────
// Ưu tiên: user session key → default key (DB) → env var GROQ_API_KEY

async fn get_api_key(state: &AppState, session_id: Option<&str>) -> Option<String> {
    // 1. User session key
    if let Some(sid) = session_id {
        if let Ok(Some(key)) = sqlx::query_scalar::<_, String>(
            "SELECT value FROM settings WHERE key = $1"
        )
        .bind(format!("groq_key_{}", sid))
        .fetch_optional(&state.db)
        .await {
            if !key.is_empty() { return Some(key); }
        }
    }
    // 2. Default key từ DB
    if let Ok(Some(key)) = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'groq_api_key'"
    )
    .fetch_optional(&state.db)
    .await {
        if !key.is_empty() { return Some(key); }
    }
    // 3. Env var fallback
    std::env::var("GROQ_API_KEY").ok()
}

// ── Save / check API key ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaveApiKeyBody {
    pub session_id: Option<String>,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct CheckApiKeyQuery {
    pub session_id: Option<String>,
}

/// POST /api/settings/groq-key
/// - session_id = None → lưu default key (admin)
/// - session_id = Some → lưu key riêng cho user đó
pub async fn save_groq_api_key(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SaveApiKeyBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let key = body.api_key.trim().to_string();
    if key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "API key không được để trống".to_string()));
    }
    let db_key = match &body.session_id {
        Some(sid) => format!("groq_key_{}", sid),
        None => "groq_api_key".to_string(),
    };
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value"
    )
    .bind(&db_key).bind(&key)
    .execute(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "status": "saved" })))
}

/// GET /api/settings/groq-key?session_id=xxx
pub async fn check_groq_api_key(
    State(state): State<Arc<AppState>>,
    Query(q): Query<CheckApiKeyQuery>,
) -> Json<serde_json::Value> {
    let has_custom_key = if let Some(sid) = &q.session_id {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM settings WHERE key = $1"
        )
        .bind(format!("groq_key_{}", sid))
        .fetch_one(&state.db).await.unwrap_or(0) > 0
    } else { false };

    let has_default_key = get_api_key(&state, None).await.is_some();

    Json(json!({ "has_custom_key": has_custom_key, "has_default_key": has_default_key }))
}

// ── Classify intent ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ClassifyIntentBody {
    pub user_prompt: String,
    pub session_id: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

pub async fn classify_intent(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClassifyIntentBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let api_key = get_api_key(&state, body.session_id.as_deref()).await
        .ok_or((StatusCode::BAD_REQUEST, "Groq API key chưa được cấu hình. Vui lòng nhập API key.".to_string()))?;

    let base_url = body.base_url.unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
    let model = body.model.unwrap_or_else(|| "llama-3.3-70b-versatile".to_string());

    let system_prompt = r#"Bạn là classifier phân tích ý định người dùng trên K2 Marketplace.

Trả về JSON CHÍNH XÁC theo schema sau, không thêm bất kỳ field nào khác:
{
  "action": "buy" | "sell" | "exchange" | "none",
  "topic": "Digital Assets" | "Goods" | "Freelance Job" | null,
  "subtopic": string | null,
  "sub_category": string | null,
  "category": string | null,
  "skill": string | null,
  "title": string,
  "description": string,
  "needs_search": boolean
}

Cấu trúc phân cấp (topic → subtopic → sub_category):

Digital Assets:
- Video → [Short Clips, Full Movies, Tutorials, Stock Footage, Animations, Live Streams]
- Images → [Photography, Illustrations, Vector Art, Icons & UI Kits, Wallpapers, NFT Artwork]
- Audio → [Music Tracks, Sound Effects, Podcasts, Voice Overs, Samples & Loops, ASMR]
- Token → [ERC-20 Tokens, NFTs, Game Tokens, Utility Tokens, Governance Tokens, Stablecoins]
- License | Key | Secret → [Software Licenses, API Keys, Game Keys, Subscription Access, Domain Access, SSL Certificates]
- Document → [Templates, Research Papers, E-Books, Legal Documents, Business Plans, Whitepapers]
- Source Code → [Full Projects, Scripts & Snippets, Libraries, Plugins, Themes, Bots & Automation]
- Dataset → [Training Data, Financial Data, Market Research, User Behavior, Geospatial Data, Medical Records]

Goods:
- Fashion → [Clothing, Shoes & Footwear, Accessories, Bags & Luggage, Jewelry, Vintage & Luxury]
- Electronics & Devices → [Smartphones, Laptops & PCs, Cameras, Audio Equipment, Gaming Gear, Smart Home]
- Books & Learning → [Fiction, Non-Fiction, Textbooks, Magazines, Comics & Manga, Study Materials]
- Sports & Travel → [Sports Equipment, Outdoor Gear, Travel Accessories, Fitness, Cycling, Water Sports]
- Toys & Games → [Rubik's Cube & Speed Cubes, Action Figures & Collectibles, Board Games & Card Games, LEGO & Building Blocks, Remote Control Toys, Puzzles, Stuffed Animals & Plushies, Educational Toys, Diecast & Model Cars, Trading Card Games (TCG), Anime & Manga Figures]
- Home & Living → [Kitchen & Cooking, Furniture & Decor, Bedding & Pillows, Bathroom Essentials, Cleaning & Organizers, Lighting, Plants & Gardening, Air Purifiers & Fans, Rice Cookers & Small Appliances, Storage & Shelving, Wall Art & Frames, Candles & Aromatherapy]

Freelance Job (dùng category thay subtopic):
- Tech & IT → [Web & Mobile Development, Software / App Development, Data Science / Analytics, IT Support / Networking]
- Design & Creative → [Graphic Design, UI/UX Design, Illustration / Animation, Video & Photo Editing]
- Writing & Translation → [Content Writing / Copywriting, Blogging / Articles, Translation / Localization, Technical Writing]
- Marketing & Sales → [Digital Marketing, Social Media Management, SEO / SEM, Sales & Lead Generation]

Ví dụ mapping:
- "mua iPhone" → topic=Goods, subtopic=Electronics & Devices, sub_category=Smartphones
- "bán laptop gaming" → topic=Goods, subtopic=Electronics & Devices, sub_category=Laptops & PCs
- "tìm rubik cube" → topic=Goods, subtopic=Toys & Games, sub_category=Rubik's Cube & Speed Cubes
- "bán máy xay sinh tố" → topic=Goods, subtopic=Home & Living, sub_category=Rice Cookers & Small Appliances
- "bán bộ truyện" → topic=Goods, subtopic=Books & Learning, sub_category=Fiction
- "bán stock footage" → topic=Digital Assets, subtopic=Video, sub_category=Stock Footage
- "thuê UI/UX designer" → topic=Freelance Job, category=Design & Creative, skill=UI/UX Design

Quy tắc:
- action="none" nếu chỉ hỏi thông tin
- needs_search=true CHỈ KHI có từ "tìm", "xem", "có ai", "danh sách", "list"
- needs_search=false khi muốn MUA/BÁN/TRAO ĐỔI
- Cố gắng điền sub_category nếu có thể suy ra từ context
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
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send().await
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

// ── Groq chat ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GroqChatBody {
    pub messages: serde_json::Value,
    pub tools: Option<serde_json::Value>,
    pub model: Option<String>,
    pub session_id: Option<String>,
}

pub async fn groq_chat_with_tools(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GroqChatBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let api_key = get_api_key(&state, body.session_id.as_deref()).await
        .ok_or((StatusCode::BAD_REQUEST, "Groq API key chưa được cấu hình. Vui lòng nhập API key.".to_string()))?;

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
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send().await
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

// ── K2 endpoint ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ClassifyK2Body {
    pub user_prompt: String,
}

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
