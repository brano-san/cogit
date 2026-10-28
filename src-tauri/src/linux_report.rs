use std::time::Duration;

const GRAPHICS_LIBS: [&str; 9] = [
    "libwebkit2gtk",
    "libjavascriptcoregtk",
    "libEGL",
    "libGLX",
    "libGL.",
    "libgbm",
    "libdrm",
    "nvidia",
    "dri/",
];

pub fn log_at_start() {
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| "-".to_owned());
    tracing::info!(
        os = os_name(&read("/etc/os-release")).unwrap_or("-"),
        kernel = read("/proc/sys/kernel/osrelease").trim(),
        desktop = %var("XDG_CURRENT_DESKTOP"),
        session = %var("XDG_SESSION_TYPE"),
        appimage = %var("APPIMAGE"),
        cpus = std::thread::available_parallelism().map_or(0, std::num::NonZero::get),
        mem_total_mb = meminfo_kb(&read("/proc/meminfo"), "MemTotal").unwrap_or(0) / 1024,
        mem_available_mb = meminfo_kb(&read("/proc/meminfo"), "MemAvailable").unwrap_or(0) / 1024,
        loadavg = read("/proc/loadavg").trim(),
        gtk = format!("{}.{}.{}", gtk::major_version(), gtk::minor_version(), gtk::micro_version()),
        "system: linux"
    );
    for gpu in gpus() {
        tracing::info!(card = %gpu.0, vendor = %gpu.1, device = %gpu.2, driver = %gpu.3, boot_vga = %gpu.4, "system: gpu");
    }
    if let Some(line) = read("/proc/driver/nvidia/version").lines().next() {
        tracing::info!(version = line, "system: nvidia driver");
    }
    tracing::info!(libs = %graphics_libs(&read("/proc/self/maps")).join(" "), "system: graphics libraries of the UI process");
    log_monitors();
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(5));
        log_webkit_children();
    });
}

fn log_monitors() {
    use gtk::gdk::prelude::MonitorExt;
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    for index in 0..display.n_monitors() {
        let Some(monitor) = display.monitor(index) else {
            continue;
        };
        let area = monitor.geometry();
        tracing::info!(
            index,
            model = monitor.model().as_deref().unwrap_or("-"),
            geometry = format!(
                "{}x{}+{}+{}",
                area.width(),
                area.height(),
                area.x(),
                area.y()
            ),
            scale = monitor.scale_factor(),
            refresh_hz = f64::from(monitor.refresh_rate()) / 1000.0,
            primary = monitor.is_primary(),
            "system: monitor"
        );
    }
}

fn log_webkit_children() {
    let me = std::process::id().to_string();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let Ok(stat) = std::fs::read_to_string(dir.join("stat")) else {
            continue;
        };
        let Some((comm, parent)) = comm_and_parent(&stat) else {
            continue;
        };
        if parent != me || !comm.starts_with("WebKit") {
            continue;
        }
        let maps = std::fs::read_to_string(dir.join("maps")).unwrap_or_default();
        let status = std::fs::read_to_string(dir.join("status")).unwrap_or_default();
        tracing::info!(
            process = comm,
            rss_mb = meminfo_kb(&status, "VmRSS").unwrap_or(0) / 1024,
            libs = %graphics_libs(&maps).join(" "),
            "system: webkit process"
        );
    }
}

fn gpus() -> Vec<(String, String, String, String, String)> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut found: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let card = name.strip_prefix("card")?;
            if !card.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let device = entry.path().join("device");
            let read = |file: &str| {
                std::fs::read_to_string(device.join(file))
                    .map_or_else(|_| "-".to_owned(), |text| text.trim().to_owned())
            };
            let driver = std::fs::read_link(device.join("driver"))
                .ok()
                .and_then(|link| {
                    link.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "-".to_owned());
            Some((
                name,
                read("vendor"),
                read("device"),
                driver,
                read("boot_vga"),
            ))
        })
        .collect();
    found.sort();
    found
}

fn graphics_libs(maps: &str) -> Vec<String> {
    let mut libs: Vec<String> = maps
        .lines()
        .filter_map(|line| line.split_whitespace().nth(5))
        .filter(|path| GRAPHICS_LIBS.iter().any(|lib| path.contains(lib)))
        .map(str::to_owned)
        .collect();
    libs.sort();
    libs.dedup();
    libs
}

fn os_name(release: &str) -> Option<&str> {
    release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|name| name.trim_matches('"'))
}

fn meminfo_kb(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kb| kb.parse().ok())
}

fn comm_and_parent(stat: &str) -> Option<(&str, &str)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let parent = stat.get(close + 1..)?.split_whitespace().nth(1)?;
    Some((stat.get(open + 1..close)?, parent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_distribution_memory_and_the_parent_of_a_process() {
        assert_eq!(
            os_name("NAME=Arch\nPRETTY_NAME=\"CachyOS Linux\"\n"),
            Some("CachyOS Linux")
        );
        assert_eq!(
            meminfo_kb(
                "MemTotal:       32768000 kB\nMemAvailable: 100 kB\n",
                "MemAvailable"
            ),
            Some(100)
        );
        assert_eq!(
            comm_and_parent("4242 (WebKit Web Pro) S 1000 4242"),
            Some(("WebKit Web Pro", "1000"))
        );
    }

    #[test]
    fn keeps_only_the_graphics_libraries_once() {
        let maps = "7f00-7f01 r-xp 0 08:01 1 /usr/lib/libEGL_nvidia.so.0\n\
                    7f01-7f02 r-xp 0 08:01 1 /usr/lib/libEGL_nvidia.so.0\n\
                    7f02-7f03 r-xp 0 08:01 2 /usr/lib/libc.so.6\n\
                    7f03-7f04 r-xp 0 08:01 3 /tmp/.mount_Cogit/usr/lib/libwebkit2gtk-4.1.so.0\n";
        assert_eq!(
            graphics_libs(maps),
            [
                "/tmp/.mount_Cogit/usr/lib/libwebkit2gtk-4.1.so.0",
                "/usr/lib/libEGL_nvidia.so.0"
            ]
        );
    }
}
