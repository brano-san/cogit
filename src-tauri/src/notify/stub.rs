//! No system notifications yet off Windows: freedesktop (D-Bus) plugs in here.

pub fn register_app_id(_id: &str) {}

pub fn show(
    _app: &tauri::AppHandle,
    _title: &str,
    _body: &str,
    _on_click: impl Fn() + Send + Sync + 'static,
) -> Result<(), String> {
    Ok(())
}
