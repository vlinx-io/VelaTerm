//! Update check that honors the selected server region.
//!
//! The updater plugin's JS `check()` always uses the endpoint in tauri.conf.json (velaterm.com). This command
//! runs the same check through the plugin's Rust builder with the endpoint of the selected region, points the
//! package download at that region's mirror, and registers the result in the webview's resource table exactly
//! as the plugin does. The frontend wraps the returned metadata in the plugin's `Update` class, so download,
//! signature verification, install and the Windows path in `updater_win.rs` all run unchanged.

use serde::Serialize;
use tauri::{Manager, ResourceId, Webview};
use tauri_plugin_updater::UpdaterExt;

/// Same template as `plugins.updater.endpoints` in tauri.conf.json, on the selected origin.
const ENDPOINT_PATH: &str = "/api/updater/{{target}}/{{arch}}?current_version={{current_version}}";

/// The plugin's check result, in the shape its JS `Update` constructor expects.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    rid: ResourceId,
    current_version: String,
    version: String,
    date: Option<String>,
    body: Option<String>,
    raw_json: serde_json::Value,
}

/// Check for a newer release on the selected region's update service. Async: it is a network request.
#[tauri::command]
pub async fn updater_check(
    webview: Webview,
    headers: Option<Vec<(String, String)>>,
) -> Result<Option<UpdateMetadata>, String> {
    let region = crate::server_region::current();
    let endpoint = format!("{}{ENDPOINT_PATH}", region.site_origin());
    let endpoint = endpoint.parse().map_err(|e| format!("Invalid update endpoint: {e}"))?;
    let mut builder = webview
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Update check failed: {e}"))?;
    for (key, value) in headers.unwrap_or_default() {
        builder = builder.header(key, value).map_err(|e| format!("Update check failed: {e}"))?;
    }
    let updater = builder.build().map_err(|e| format!("Update check failed: {e}"))?;
    let Some(mut update) = updater.check().await.map_err(|e| format!("Update check failed: {e}"))? else {
        return Ok(None);
    };

    // The manifest lists dl.velaterm.com; the China mirror holds the same signed file under the same key.
    let localized = region.localize_download_url(update.download_url.as_str());
    if localized != update.download_url.as_str() {
        update.download_url = localized.parse().map_err(|e| format!("Invalid update download URL: {e}"))?;
    }

    let date = update
        .date
        .and_then(|d| d.format(&time::format_description::well_known::Rfc3339).ok());
    let metadata = UpdateMetadata {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        date,
        body: update.body.clone(),
        raw_json: update.raw_json.clone(),
        rid: 0,
    };
    let rid = webview.resources_table().add(update);
    Ok(Some(UpdateMetadata { rid, ..metadata }))
}
