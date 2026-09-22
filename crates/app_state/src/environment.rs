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
