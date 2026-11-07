//! Palettes for the retro look: Lospec-style `.hex` files in `assets/palettes/`, one
//! `rrggbb` per line (the format lospec.com exports, so any palette there drops straight in).

use bevy::color::{Color, LinearRgba, Oklaba, Srgba};
use bevy::math::Vec4;

pub const PALETTE_DIR: &str = "assets/palettes";

/// The shader's palette arrays hold this many colours; longer palettes are cut.
pub const MAX_COLORS: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    /// File stem, e.g. `obra-1bit`.
    pub name: String,
    pub colors: Vec<Srgba>,
}

impl Palette {
    /// The colours as the shader wants them: OKLab (to match against) and linear RGB (to
    /// write out), both padded to `MAX_COLORS`.
    pub fn shader_arrays(&self) -> ([Vec4; MAX_COLORS], [Vec4; MAX_COLORS]) {
        let mut lab = [Vec4::ZERO; MAX_COLORS];
        let mut linear = [Vec4::ZERO; MAX_COLORS];
        for ((c, lab), linear) in self.colors.iter().zip(&mut lab).zip(&mut linear) {
            let lin: LinearRgba = (*c).into();
            let ok: Oklaba = Color::from(lin).into();
            *lab = Vec4::new(ok.lightness, ok.a, ok.b, 1.0);
            *linear = Vec4::new(lin.red, lin.green, lin.blue, 1.0);
        }
        (lab, linear)
    }

    pub fn len(&self) -> usize {
        self.colors.len().min(MAX_COLORS)
    }
}

/// Parse a `.hex` palette: one colour per line, `#` optional, blank lines and `;`/`//`
/// comments skipped. `None` if a line isn't a colour or there are no colours.
pub fn parse_hex(name: &str, text: &str) -> Option<Palette> {
    let mut colors = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
            continue;
        }
        colors.push(Srgba::hex(line.trim_start_matches('#')).ok()?);
    }
    (!colors.is_empty()).then(|| Palette {
        name: name.to_string(),
        colors,
    })
}

/// Every palette in `PALETTE_DIR`, sorted by name. Unreadable files are logged and skipped.
pub fn load_all() -> Vec<Palette> {
    let Ok(dir) = std::fs::read_dir(PALETTE_DIR) else {
        bevy::log::warn!("no palettes: cannot read {PALETTE_DIR}");
        return Vec::new();
    };
    let mut palettes: Vec<Palette> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "hex"))
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).ok()?;
            let palette = parse_hex(&name, &text);
            if palette.is_none() {
                bevy::log::warn!("skipping palette {}: not a .hex palette", path.display());
            }
            palette
        })
        .collect();
    palettes.sort_by(|a, b| a.name.cmp(&b.name));
    palettes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lospec_hex_with_or_without_hashes_and_comments() {
        let p = parse_hex("t", "; my palette\nff0000\n\n#00ff00\n// x\n0000FF\n").unwrap();
        assert_eq!(p.colors.len(), 3);
        assert_eq!(p.colors[1], Srgba::rgb_u8(0, 255, 0));
    }

    #[test]
    fn rejects_garbage_and_empty_files() {
        assert!(parse_hex("t", "ff0000\nnot a colour\n").is_none());
        assert!(parse_hex("t", "\n; nothing\n").is_none());
    }

    #[test]
    fn shader_arrays_put_white_at_full_lightness() {
        let p = parse_hex("t", "000000\nffffff\n").unwrap();
        let (lab, linear) = p.shader_arrays();
        assert!(lab[0].x.abs() < 1e-3);
        assert!((lab[1].x - 1.0).abs() < 1e-3);
        assert_eq!(linear[1], Vec4::ONE);
        assert_eq!(lab[2], Vec4::ZERO, "padding stays zero");
    }

    #[test]
    fn every_palette_in_assets_parses() {
        let names: Vec<_> = std::fs::read_dir(PALETTE_DIR)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "hex"))
            .collect();
        assert!(!names.is_empty());
        for path in names {
            let text = std::fs::read_to_string(&path).unwrap();
            let p = parse_hex("t", &text).unwrap_or_else(|| panic!("{} parses", path.display()));
            assert!(
                p.colors.len() <= MAX_COLORS,
                "{} has too many colours",
                path.display()
            );
        }
    }
}
