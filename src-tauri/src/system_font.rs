//! The desktop's UI font the page starts from, and what the display does to text, logged
//! once at start so a blurry or small UI can be told apart from a scaled one (R-775).

use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemFont {
    /// The desktop's UI family: `gtk-font-name` on Linux, Segoe UI on Windows.
    pub family: Option<String>,
    /// Its size in CSS pixels.
    pub size_px: Option<f64>,
    /// Of the families the page asked about, those installed here; `None` where the platform
    /// is not asked (Windows, macOS ship the families their stacks name).
    pub installed: Option<Vec<String>>,
}

static DESKTOP: OnceLock<(Option<String>, Option<f64>)> = OnceLock::new();

/// On the main thread, before the page asks: GTK is read only there.
pub fn init<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let font = desktop_font();
    let _ = DESKTOP.set(font.clone());
    let scale = window.scale_factor().unwrap_or(1.0);
    tracing::info!(
        scale_factor = scale,
        family = font.0.as_deref().unwrap_or("-"),
        size_px = font.1.unwrap_or(0.0),
        "display: window scale and desktop font"
    );
    #[cfg(target_os = "linux")]
    log_gtk();
    #[cfg(target_os = "linux")]
    if let Some(family) = font.0 {
        std::thread::spawn(move || log_fc_match(&family));
    }
}

#[cfg(windows)]
fn desktop_font() -> (Option<String>, Option<f64>) {
    // ponytail: the shell's message font is Segoe UI 9pt on every Windows 10/11 locale but
    // East Asian ones; read NONCLIENTMETRICS when one of those needs its own.
    (Some("Segoe UI".to_owned()), Some(12.0))
}

#[cfg(target_os = "linux")]
fn desktop_font() -> (Option<String>, Option<f64>) {
    use gtk::prelude::GtkSettingsExt;
    let named = gtk::Settings::default().and_then(|settings| settings.gtk_font_name());
    match named.as_deref().and_then(app_state::environment::css_font) {
        Some((family, px)) => (Some(family), Some(px)),
        None => (None, None),
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn desktop_font() -> (Option<String>, Option<f64>) {
    (None, None)
}

#[cfg(target_os = "linux")]
fn log_gtk() {
    use gtk::gdk::prelude::MonitorExt;
    use gtk::glib::prelude::ObjectExt;
    use gtk::prelude::GtkSettingsExt;
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| "-".to_owned());
    let display = gtk::gdk::Display::default();
    let backend = display.as_ref().map_or_else(
        || "-".to_owned(),
        |display| display.type_().name().to_owned(),
    );
    let monitor_scale = display
        .as_ref()
        .and_then(gtk::gdk::Display::primary_monitor)
        .map_or(0, |monitor| monitor.scale_factor());
    let settings = gtk::Settings::default();
    let xft_dpi = settings.as_ref().map_or(0, GtkSettingsExt::gtk_xft_dpi);
    tracing::info!(
        gdk_backend_env = %var("GDK_BACKEND"),
        gdk_display = %backend,
        gdk_monitor_scale = monitor_scale,
        gdk_scale = %var("GDK_SCALE"),
        gdk_dpi_scale = %var("GDK_DPI_SCALE"),
        wayland_display = %var("WAYLAND_DISPLAY"),
        x_display = %var("DISPLAY"),
        xft_dpi = f64::from(xft_dpi) / 1024.0,
        font_name = settings.as_ref().and_then(GtkSettingsExt::gtk_font_name).as_deref().unwrap_or("-"),
        xft_antialias = settings.as_ref().map_or(-1, GtkSettingsExt::gtk_xft_antialias),
        xft_hinting = settings.as_ref().map_or(-1, GtkSettingsExt::gtk_xft_hinting),
        xft_hintstyle = settings.as_ref().and_then(GtkSettingsExt::gtk_xft_hintstyle).as_deref().unwrap_or("-"),
        xft_rgba = settings.as_ref().and_then(GtkSettingsExt::gtk_xft_rgba).as_deref().unwrap_or("-"),
        nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists(),
        vmwgfx = std::path::Path::new("/sys/module/vmwgfx").exists(),
        webkit_disable_dmabuf_renderer = %var("WEBKIT_DISABLE_DMABUF_RENDERER"),
        webkit_disable_compositing_mode = %var("WEBKIT_DISABLE_COMPOSITING_MODE"),
        nv_disable_explicit_sync = %var("__NV_DISABLE_EXPLICIT_SYNC"),
        "display: GTK"
    );
}

/// What fontconfig hands WebKitGTK for the family: the file, antialiasing and hinting.
#[cfg(target_os = "linux")]
fn log_fc_match(family: &str) {
    let format = "%{family[0]} %{file} antialias=%{antialias} hinting=%{hinting} hintstyle=%{hintstyle} rgba=%{rgba}";
    match std::process::Command::new("fc-match")
        .args(["--format", format, family])
        .output()
    {
        Ok(out) => tracing::info!(
            family,
            matched = %String::from_utf8_lossy(&out.stdout),
            "display: fc-match"
        ),
        Err(err) => tracing::warn!(error = %err, family, "display: fc-match did not run"),
    }
}

/// The desktop font, and which of `families` are installed (Linux, through `fc-list`).
#[must_use]
pub fn read(families: &[String]) -> SystemFont {
    let (family, size_px) = DESKTOP.get().cloned().unwrap_or_default();
    SystemFont {
        family,
        size_px,
        installed: installed(families),
    }
}

/// Every installed family, sorted and once each.
#[must_use]
pub fn families() -> Vec<String> {
    let mut all = listed();
    all.sort_by_key(|family| family.to_lowercase());
    all.dedup();
    all
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn listed() -> Vec<String> {
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
    };
    // SAFETY: COM calls on objects created here; a shared DirectWrite factory needs no apartment.
    let read = || -> windows::core::Result<Vec<String>> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let mut collection = None;
            factory.GetSystemFontCollection(&mut collection, false)?;
            let Some(collection) = collection else {
                return Ok(Vec::new());
            };
            let mut all = Vec::new();
            for i in 0..collection.GetFontFamilyCount() {
                let names = collection.GetFontFamily(i)?.GetFamilyNames()?;
                let (mut index, mut english) = (0, windows::core::BOOL(0));
                names.FindLocaleName(windows::core::w!("en-us"), &mut index, &mut english)?;
                let index = if english.as_bool() { index } else { 0 };
                let mut name = vec![0u16; names.GetStringLength(index)? as usize + 1];
                names.GetString(index, &mut name)?;
                name.pop();
                all.push(String::from_utf16_lossy(&name));
            }
            Ok(all)
        }
    };
    read().unwrap_or_else(|err| {
        tracing::warn!(%err, "font families not listed");
        Vec::new()
    })
}

/// A line of `fc-list` holds one font's family names, comma-separated; the first is the one
/// CSS knows it by.
#[cfg(target_os = "linux")]
fn listed() -> Vec<String> {
    let Ok(out) = std::process::Command::new("fc-list")
        .args([
            "--format",
            "%{family}
",
        ])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split(',').next())
        .map(str::trim)
        .filter(|family| !family.is_empty())
        .map(str::to_owned)
        .collect()
}

// ponytail: macOS lists nothing yet; NSFontManager.availableFontFamilies when it ships.
#[cfg(not(any(windows, target_os = "linux")))]
fn listed() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn installed(families: &[String]) -> Option<Vec<String>> {
    let out = std::process::Command::new("fc-list")
        .args(["--format", "%{family}\n"])
        .output()
        .map_err(|err| tracing::warn!(error = %err, "fc-list did not run; the stack stays whole"))
        .ok()?;
    Some(app_state::environment::installed_families(
        &String::from_utf8_lossy(&out.stdout),
        families,
    ))
}

#[cfg(not(target_os = "linux"))]
fn installed(_: &[String]) -> Option<Vec<String>> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn windows_lists_its_ui_font_once() {
        let all = super::families();
        assert_eq!(all.iter().filter(|family| *family == "Segoe UI").count(), 1);
    }
}
