//! What the taskbar button shows: the pure merge of the page's signals and the overlay icons
//! drawn in code, so every platform's answer is tested on any machine (R-638).

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Progress {
    Indeterminate,
    Percent { value: u8 },
}

/// Blinks until the window is focused: only an error or a warning asks for attention. A
/// success never flashes — a short flash cannot be stopped on focus (`FLASHW_TRAY` without
/// `FLASHW_TIMERNOFG` blinks its count out) and reads as an alarm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Flash {
    Persistent,
}

/// Everything the page knows; the host decides what wins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskbarSignals {
    pub progress: Option<Progress>,
    /// Failed commands still waiting in the notification window.
    pub errors: u32,
    /// Repository warnings (conflicts) still unresolved.
    pub warnings: u32,
    /// Events that ended while the window was in the background.
    pub unviewed: u32,
    /// Ask for attention now; honored only while the window is not focused.
    pub flash: Option<Flash>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    None,
    Normal(u8),
    Indeterminate,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Error,
    Warning,
    Count(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appearance {
    pub bar: Bar,
    pub overlay: Overlay,
    pub flash: Option<Flash>,
}

/// Priority: error, then warning, then progress. A warning keeps a running bar.
#[must_use]
pub fn resolve(signals: &TaskbarSignals) -> Appearance {
    let bar = match signals.progress {
        Some(Progress::Percent { value }) => Bar::Normal(value.min(100)),
        Some(Progress::Indeterminate) => Bar::Indeterminate,
        None => Bar::None,
    };
    let (bar, overlay) = if signals.errors > 0 {
        (Bar::Error, Overlay::Error)
    } else if signals.warnings > 0 {
        (bar, Overlay::Warning)
    } else if signals.unviewed >= 2 {
        (bar, Overlay::Count(signals.unviewed))
    } else {
        (bar, Overlay::None)
    };
    Appearance {
        bar,
        overlay,
        flash: signals.flash,
    }
}

pub const ICON_SIZE: u32 = 16;

const RED: [u8; 3] = [0xD1, 0x34, 0x38];
const YELLOW: [u8; 3] = [0xF2, 0xB1, 0x00];
const BLUE: [u8; 3] = [0x1F, 0x6F, 0xEB];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const DARK: [u8; 3] = [0x20, 0x20, 0x20];

/// 3x5 glyphs, one row per entry, high bit left.
fn glyph(ch: char) -> [u8; 5] {
    match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        _ => [0b010, 0b010, 0b010, 0b000, 0b010],
    }
}

/// A 16x16 RGBA disk with its text; `None` for no overlay. Above 9 the badge reads `9+`.
#[must_use]
pub fn overlay_rgba(overlay: Overlay) -> Option<Vec<u8>> {
    let (text, disk, ink) = match overlay {
        Overlay::None => return None,
        Overlay::Error => ("!".to_owned(), RED, WHITE),
        Overlay::Warning => ("!".to_owned(), YELLOW, DARK),
        Overlay::Count(n) if n > 9 => ("9+".to_owned(), BLUE, WHITE),
        Overlay::Count(n) => (n.to_string(), BLUE, WHITE),
    };
    let size = ICON_SIZE as usize;
    let mut rgba = vec![0u8; size * size * 4];
    let centre = (size as f32 - 1.0) / 2.0;
    for y in 0..size {
        for x in 0..size {
            let distance = ((x as f32 - centre).powi(2) + (y as f32 - centre).powi(2)).sqrt();
            let coverage = (size as f32 / 2.0 + 0.5 - distance).clamp(0.0, 1.0);
            let at = (y * size + x) * 4;
            rgba[at..at + 3].copy_from_slice(&disk);
            rgba[at + 3] = (coverage * 255.0) as u8;
        }
    }
    // Scale 2: a glyph is 6x10, one gap pixel between glyphs, centred.
    let glyphs: Vec<[u8; 5]> = text.chars().map(glyph).collect();
    let width = glyphs.len() * 6 + glyphs.len().saturating_sub(1);
    let left = (size - width) / 2;
    let top = (size - 10) / 2;
    for (index, rows) in glyphs.iter().enumerate() {
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..3 {
                if bits >> (2 - col) & 1 == 0 {
                    continue;
                }
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let x = left + index * 7 + col * 2 + dx;
                    let y = top + row * 2 + dy;
                    let at = (y * size + x) * 4;
                    rgba[at..at + 3].copy_from_slice(&ink);
                }
            }
        }
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals() -> TaskbarSignals {
        TaskbarSignals::default()
    }

    #[test]
    fn an_idle_page_shows_nothing() {
        let shown = resolve(&signals());
        assert_eq!(
            (shown.bar, shown.overlay, shown.flash),
            (Bar::None, Overlay::None, None)
        );
    }

    #[test]
    fn an_error_beats_a_warning_and_a_running_operation() {
        let shown = resolve(&TaskbarSignals {
            progress: Some(Progress::Percent { value: 40 }),
            errors: 1,
            warnings: 2,
            unviewed: 5,
            ..signals()
        });
        assert_eq!((shown.bar, shown.overlay), (Bar::Error, Overlay::Error));
    }

    #[test]
    fn a_warning_keeps_the_running_bar() {
        let shown = resolve(&TaskbarSignals {
            progress: Some(Progress::Indeterminate),
            warnings: 1,
            unviewed: 3,
            ..signals()
        });
        assert_eq!(
            (shown.bar, shown.overlay),
            (Bar::Indeterminate, Overlay::Warning)
        );
    }

    #[test]
    fn a_percent_over_100_is_clamped() {
        let shown = resolve(&TaskbarSignals {
            progress: Some(Progress::Percent { value: 250 }),
            ..signals()
        });
        assert_eq!(shown.bar, Bar::Normal(100));
    }

    #[test]
    fn one_unviewed_event_has_no_number_and_two_do() {
        assert_eq!(
            resolve(&TaskbarSignals {
                unviewed: 1,
                ..signals()
            })
            .overlay,
            Overlay::None
        );
        assert_eq!(
            resolve(&TaskbarSignals {
                unviewed: 2,
                ..signals()
            })
            .overlay,
            Overlay::Count(2)
        );
    }

    #[test]
    fn no_overlay_has_no_icon_and_the_rest_are_16_by_16() {
        assert!(overlay_rgba(Overlay::None).is_none());
        for overlay in [
            Overlay::Error,
            Overlay::Warning,
            Overlay::Count(3),
            Overlay::Count(42),
        ] {
            assert_eq!(
                overlay_rgba(overlay).map(|icon| icon.len()),
                Some(16 * 16 * 4)
            );
        }
    }

    #[test]
    fn the_icon_is_a_disk_with_ink_inside() {
        let icon = overlay_rgba(Overlay::Error).unwrap();
        let pixel = |x: usize, y: usize| &icon[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
        assert_eq!(pixel(0, 0)[3], 0, "corner is transparent");
        assert_eq!(
            pixel(3, 8),
            &[0xD1, 0x34, 0x38, 255],
            "disk colour off the glyph"
        );
        assert_eq!(
            pixel(7, 3),
            &[255, 255, 255, 255],
            "the bar of the exclamation mark"
        );
    }

    #[test]
    fn different_counts_draw_different_icons() {
        assert_ne!(
            overlay_rgba(Overlay::Count(2)),
            overlay_rgba(Overlay::Count(3))
        );
        assert_eq!(
            overlay_rgba(Overlay::Count(10)),
            overlay_rgba(Overlay::Count(99))
        );
    }
}
