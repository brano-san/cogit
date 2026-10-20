use app_state::environment::{os_info, renderer_label, unix_product, windows_names};

fn names(product_name: &str, build: u32) -> (String, Option<String>) {
    windows_names(product_name, build)
}

#[test]
fn windows_11_is_named_by_its_build_because_the_registry_still_says_10() {
    assert_eq!(
        names("Windows 10 Home", 26100),
        ("Windows 11".to_owned(), Some("Home".to_owned()))
    );
}

#[test]
fn windows_10_keeps_its_name() {
    assert_eq!(
        names("Windows 10 Pro", 19045),
        ("Windows 10".to_owned(), Some("Pro".to_owned()))
    );
}

#[test]
fn a_registry_that_already_says_11_is_taken_at_its_word() {
    assert_eq!(
        names("Windows 11 Pro", 26100),
        ("Windows 11".to_owned(), Some("Pro".to_owned()))
    );
}

#[test]
fn a_multi_word_edition_is_kept_whole() {
    assert_eq!(
        names("Windows 10 Enterprise LTSC 2021", 19044),
        (
            "Windows 10".to_owned(),
            Some("Enterprise LTSC 2021".to_owned())
        )
    );
}

#[test]
fn a_server_keeps_its_year_in_the_product() {
    assert_eq!(
        names("Windows Server 2022 Datacenter", 20348),
        (
            "Windows Server 2022".to_owned(),
            Some("Datacenter".to_owned())
        )
    );
}

#[test]
fn a_missing_product_name_still_reads_as_windows() {
    assert_eq!(names("", 26100), ("Windows 11".to_owned(), None));
    assert_eq!(names("", 9600), ("Windows".to_owned(), None));
}

#[test]
fn macos_is_named_with_its_product_version() {
    assert_eq!(
        unix_product("macos", Some("Darwin"), Some("15.1.1")),
        "macOS 15.1.1"
    );
}

#[test]
fn a_linux_distribution_is_named_with_its_release() {
    assert_eq!(
        unix_product("linux", Some("Ubuntu"), Some("24.04")),
        "Ubuntu 24.04"
    );
}

#[test]
fn a_linux_without_os_release_is_still_linux() {
    assert_eq!(unix_product("linux", None, None), "Linux");
}

#[test]
fn the_renderer_is_named_for_the_platform_it_runs_on() {
    assert_eq!(
        renderer_label("windows", Some("153.0.4234.48")),
        "WebView2 153.0.4234.48"
    );
    assert_eq!(renderer_label("linux", Some("2.44.0")), "WebKitGTK 2.44.0");
    assert_eq!(
        renderer_label("macos", Some("620.1.15")),
        "WKWebView 620.1.15"
    );
}

#[test]
fn a_renderer_that_did_not_answer_says_so() {
    assert_eq!(
        renderer_label("windows", None),
        "WebView2 (version unknown)"
    );
}

#[test]
fn this_machine_reports_a_product_and_the_architecture_cogit_was_built_for() {
    let os = os_info();
    assert!(!os.product.is_empty());
    assert_eq!(os.arch, std::env::consts::ARCH);
}

#[cfg(windows)]
#[test]
fn this_windows_reports_a_numeric_build() {
    let os = os_info();
    assert!(os.product.starts_with("Windows"), "{os:?}");
    let build = os.build.expect("the registry holds CurrentBuildNumber");
    assert!(build.parse::<u32>().is_ok(), "{build}");
}

// Cogit drew its own stack at 13px while SmartGit took the desktop's font: the system font
// is what the UI starts from.
#[test]
fn a_gtk_font_name_becomes_a_css_family_and_size() {
    use app_state::environment::css_font;
    assert_eq!(
        css_font("Ubuntu 11"),
        Some(("Ubuntu".to_owned(), 11.0 * 4.0 / 3.0))
    );
    assert_eq!(
        css_font("Noto Sans Medium 10"),
        Some(("Noto Sans".to_owned(), 10.0 * 4.0 / 3.0))
    );
    assert_eq!(
        css_font("Cantarell, Sans Bold Italic 12"),
        Some(("Cantarell".to_owned(), 16.0))
    );
    assert_eq!(
        css_font("DejaVu Sans 14px"),
        Some(("DejaVu Sans".to_owned(), 14.0))
    );
}

#[test]
fn a_gtk_font_name_without_a_family_or_a_size_says_nothing() {
    use app_state::environment::css_font;
    assert_eq!(css_font("12"), None);
    assert_eq!(css_font("Ubuntu"), None);
    assert_eq!(css_font(""), None);
}

#[test]
fn only_installed_families_stay_in_the_fallback_stack() {
    use app_state::environment::installed_families;
    let listed = "DejaVu Sans\nNoto Sans,Noto Sans Regular\nUbuntu Mono\n";
    let wanted = ["Inter", "Noto Sans", "DejaVu Sans", "Cantarell"].map(String::from);
    assert_eq!(
        installed_families(listed, &wanted),
        ["Noto Sans", "DejaVu Sans"]
    );
}
