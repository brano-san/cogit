//! What About and Copy Diagnostics say about the machine; the frontend only formats it.

use serde::Serialize;

/// The first Windows 11 build. Its registry still calls itself "Windows 10".
const FIRST_WINDOWS_11_BUILD: u32 = 22000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    pub product: String,
    pub edition: Option<String>,
    pub release: Option<String>,
    pub build: Option<String>,
    /// The cumulative update on top of the build: `4946` in `26100.4946`.
    pub revision: Option<String>,
    pub kernel: Option<String>,
    /// The architecture Cogit was compiled for, which is the one that matters for a bug.
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub primary: bool,
}

/// Product and edition from the registry's `ProductName`, corrected by the build number.
#[must_use]
pub fn windows_names(product_name: &str, build: u32) -> (String, Option<String>) {
    let fallback = || {
        if build >= FIRST_WINDOWS_11_BUILD {
            "Windows 11".to_owned()
        } else {
            "Windows".to_owned()
        }
    };

    let mut words = product_name.split_whitespace().peekable();
    if words.next() != Some("Windows") {
        return (fallback(), None);
    }
    let product = match words.next() {
        Some("Server") => match words.next_if(|word| word.chars().all(|c| c.is_ascii_digit())) {
            Some(year) => format!("Windows Server {year}"),
            None => "Windows Server".to_owned(),
        },
        Some("10") if build >= FIRST_WINDOWS_11_BUILD => "Windows 11".to_owned(),
        Some(version) => format!("Windows {version}"),
        None => fallback(),
    };
    let edition = words.collect::<Vec<_>>().join(" ");
    (product, (!edition.is_empty()).then_some(edition))
}

#[must_use]
pub fn unix_product(os: &str, name: Option<&str>, version: Option<&str>) -> String {
    let name = if os == "macos" {
        "macOS"
    } else {
        name.map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("Linux")
    };
    match version.map(str::trim).filter(|version| !version.is_empty()) {
        Some(version) => format!("{name} {version}"),
        None => name.to_owned(),
    }
}

#[must_use]
pub fn renderer_label(os: &str, version: Option<&str>) -> String {
    let name = match os {
        "windows" => "WebView2",
        "macos" | "ios" => "WKWebView",
        "android" => "Android WebView",
        _ => "WebKitGTK",
    };
    match version.map(str::trim).filter(|version| !version.is_empty()) {
        Some(version) => format!("{name} {version}"),
        None => format!("{name} (version unknown)"),
    }
}

#[must_use]
pub fn os_info() -> OsInfo {
    gather()
}

#[cfg(windows)]
fn gather() -> OsInfo {
    let key = windows_registry::LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    if let Err(err) = &key {
        tracing::warn!(error = ?err, context = "cannot read the Windows version from the registry");
    }
    let read = |name: &str| {
        key.as_ref()
            .ok()
            .and_then(|key| key.get_string(name).ok())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };

    let build = read("CurrentBuildNumber");
    let number = build
        .as_deref()
        .and_then(|build| build.parse().ok())
        .unwrap_or(0);
    let (product, edition) = windows_names(read("ProductName").as_deref().unwrap_or(""), number);
    // `DisplayVersion` arrived with 20H2; the releases before it only have `ReleaseId`.
    let release = read("DisplayVersion").or_else(|| read("ReleaseId"));
    let revision = key
        .as_ref()
        .ok()
        .and_then(|key| key.get_u32("UBR").ok())
        .map(|revision| revision.to_string());

    OsInfo {
        product,
        edition,
        release,
        build,
        revision,
        kernel: None,
        arch: std::env::consts::ARCH.to_owned(),
    }
}

#[cfg(not(windows))]
fn gather() -> OsInfo {
    OsInfo {
        product: unix_product(
            std::env::consts::OS,
            sysinfo::System::name().as_deref(),
            sysinfo::System::os_version().as_deref(),
        ),
        edition: None,
        release: None,
        build: None,
        revision: None,
        kernel: sysinfo::System::kernel_version(),
        arch: std::env::consts::ARCH.to_owned(),
    }
}

/// Pango style words that follow the family in `gtk-font-name`.
const PANGO_STYLES: &[&str] = &[
    "normal",
    "roman",
    "oblique",
    "italic",
    "small-caps",
    "thin",
    "ultra-light",
    "extra-light",
    "light",
    "semi-light",
    "demi-light",
    "book",
    "regular",
    "medium",
    "semi-bold",
    "demi-bold",
    "bold",
    "ultra-bold",
    "extra-bold",
    "heavy",
    "black",
    "ultra-heavy",
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
];

/// `gtk-font-name` (a Pango description: families, styles, size in points or `px`) as the
/// first family and a size in CSS pixels, 96 to the inch.
#[must_use]
pub fn css_font(pango: &str) -> Option<(String, f64)> {
    let mut words: Vec<&str> = pango.split_whitespace().collect();
    let size = words.pop()?;
    let px = match size.strip_suffix("px") {
        Some(px) => px.parse::<f64>().ok()?,
        None => size.parse::<f64>().ok()? * 4.0 / 3.0,
    };
    while words
        .last()
        .is_some_and(|word| PANGO_STYLES.contains(&word.to_ascii_lowercase().as_str()))
    {
        words.pop();
    }
    let families = words.join(" ");
    let family = families.split(',').next()?.trim();
    (!family.is_empty() && px > 0.0).then(|| (family.to_owned(), px))
}

/// The `wanted` families `fc-list --format '%{family}\n'` lists, in `wanted`'s order. A line
/// holds a font's family names, comma-separated.
#[must_use]
pub fn installed_families(listed: &str, wanted: &[String]) -> Vec<String> {
    let have: std::collections::HashSet<&str> = listed
        .lines()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .collect();
    wanted
        .iter()
        .filter(|family| have.contains(family.as_str()))
        .cloned()
        .collect()
}
