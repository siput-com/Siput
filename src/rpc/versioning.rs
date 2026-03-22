//! API Versioning
//!
//! This module provides API versioning utilities and constants
//! for maintaining backward compatibility and SDK stability.

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response, Json},
};
use serde::{Deserialize, Serialize};

/// Current API version
pub const CURRENT_API_VERSION: &str = "v1";

/// Supported API versions
pub const SUPPORTED_VERSIONS: &[&str] = &["v1"];

/// API version header name
pub const API_VERSION_HEADER: &str = "X-API-Version";

/// API version info response
#[derive(Serialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub supported_versions: Vec<String>,
    pub deprecated_versions: Vec<String>,
}

/// Check if API version is supported
pub fn is_version_supported(version: &str) -> bool {
    SUPPORTED_VERSIONS.contains(&version)
}

/// Get current API version info
pub fn get_version_info() -> VersionInfo {
    VersionInfo {
        current_version: CURRENT_API_VERSION.to_string(),
        supported_versions: SUPPORTED_VERSIONS.iter().map(|s| s.to_string()).collect(),
        deprecated_versions: vec![], // Add deprecated versions here when needed
    }
}

/// Middleware to handle API versioning
pub async fn version_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract version from path (e.g., /v1/endpoint)
    let path = request.uri().path();

    if let Some(version) = extract_version_from_path(path) {
        if !is_version_supported(&version) {
            return Ok((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Unsupported API version",
                    "supported_versions": SUPPORTED_VERSIONS
                }))
            ).into_response());
        }
    } else {
        // Default to current version if no version specified
        // This allows backward compatibility
    }

    Ok(next.run(request).await)
}

/// Extract version from request path
fn extract_version_from_path(path: &str) -> Option<String> {
    path.strip_prefix('/')
        .and_then(|p| p.split('/').next())
        .filter(|v| v.starts_with('v') && v.chars().skip(1).all(|c| c.is_ascii_digit()))
        .map(|v| v.to_string())
}

/// Build versioned path
pub fn versioned_path(endpoint: &str) -> String {
    format!("/{}/{}", CURRENT_API_VERSION, endpoint.trim_start_matches('/'))
}

/// Build versioned path for specific version
pub fn versioned_path_with_version(version: &str, endpoint: &str) -> String {
    format!("/{}/{}", version, endpoint.trim_start_matches('/'))
}