use axum::{http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use qrcode::QrCode;
use qrcode::render::svg;

#[derive(Deserialize)]
pub struct QrSvgBody {
    pub data: String,
}

/// POST /api/qr-svg
pub async fn generate_qr_svg(
    Json(body): Json<QrSvgBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let code = QrCode::new(body.data.as_bytes())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let svg_str = code.render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#ffffff"))
        .light_color(svg::Color("#1a1a1a"))
        .build();
    Ok(Json(json!({ "svg": svg_str })))
}
