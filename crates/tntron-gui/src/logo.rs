use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use iced::widget::image::Handle as ImageHandle;
use iced::{window, Color};

const WINDOW_ICON_PNG: &[u8] = include_bytes!("../assets/tntron-icon-64.png");
const LOGO_MASK_PNG: &[u8] = include_bytes!("../assets/tntron-logo-252.png");

static DECODED_MASK: OnceLock<Option<(u32, u32, Vec<u8>)>> = OnceLock::new();
static TINT_CACHE: OnceLock<Mutex<HashMap<[u8; 3], ImageHandle>>> = OnceLock::new();

fn base_mask() -> Option<&'static (u32, u32, Vec<u8>)> {
    DECODED_MASK
        .get_or_init(|| {
            let img = image::load_from_memory(LOGO_MASK_PNG).ok()?.to_rgba8();
            let (w, h) = img.dimensions();
            Some((w, h, img.into_raw()))
        })
        .as_ref()
}

/// Decodes the embedded 64x64 Openify/tntron badge icon for OS window managers.
pub fn window_icon() -> Option<window::Icon> {
    let img = image::load_from_memory(WINDOW_ICON_PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    window::icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Renders the bundled `tntron-logo` dissolving play mark tinted to `tint`.
pub fn tinted_logo_handle(tint: Color) -> Option<ImageHandle> {
    let (w, h, raw) = base_mask()?;
    let tr = tint.r.clamp(0.0, 1.0);
    let tg = tint.g.clamp(0.0, 1.0);
    let tb = tint.b.clamp(0.0, 1.0);
    let mut out = Vec::with_capacity(raw.len());
    for px in raw.chunks_exact(4) {
        let a = px[3];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let r = ((px[0] as f32) * tr).round().clamp(0.0, 255.0) as u8;
            let g = ((px[1] as f32) * tg).round().clamp(0.0, 255.0) as u8;
            let b = ((px[2] as f32) * tb).round().clamp(0.0, 255.0) as u8;
            out.extend_from_slice(&[r, g, b, a]);
        }
    }
    Some(ImageHandle::from_rgba(*w, *h, out))
}

/// Memoized `Arc`-backed `tinted_logo_handle` keyed by RGB8 so `view()` rebuilds stay O(1).
pub fn cached_tinted_logo_handle(tint: Color) -> Option<ImageHandle> {
    let key = [
        (tint.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (tint.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (tint.b.clamp(0.0, 1.0) * 255.0).round() as u8,
    ];
    let cache = TINT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(h) = guard.get(&key) {
            return Some(h.clone());
        }
    }
    let handle = tinted_logo_handle(tint)?;
    if let Ok(mut guard) = cache.lock() {
        if guard.len() > 64 {
            guard.clear();
        }
        guard.insert(key, handle.clone());
    }
    Some(handle)
}
