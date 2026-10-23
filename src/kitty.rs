use crate::layout::Rect;
use base64::Engine;
use std::env;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverBackend {
    Cells,
    Kitty,
}

impl CoverBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "cells" | "braille" | "halfblock" => Some(Self::Cells),
            "kitty" | "pixels" => Some(Self::Kitty),
            "auto" => Some(if Self::supported() {
                Self::Kitty
            } else {
                Self::Cells
            }),
            _ => None,
        }
    }

    pub fn supported() -> bool {
        env::var("KITTY_WINDOW_ID").is_ok()
            || env::var("TERM_PROGRAM").is_ok_and(|value| value.eq_ignore_ascii_case("ghostty"))
            || env::var("TERM").is_ok_and(|value| value.contains("kitty"))
    }
}

pub fn delete_image() -> Vec<u8> {
    b"\x1b_Ga=d,d=I,i=7001,q=2\x1b\\".to_vec()
}

fn delete_placement() -> Vec<u8> {
    b"\x1b_Ga=d,d=i,i=7001,q=2\x1b\\".to_vec()
}

#[cfg(test)]
pub fn place(rect: Rect, width: usize, height: usize, rgb: &[u8]) -> Vec<u8> {
    if rgb.len() != width.saturating_mul(height).saturating_mul(3) {
        return Vec::new();
    }
    let payload = base64::engine::general_purpose::STANDARD.encode(rgb);
    place_encoded(rect, width, height, &payload, 7001)
}

fn place_encoded(rect: Rect, width: usize, height: usize, payload: &str, image_id: u32) -> Vec<u8> {
    let inner = rect.inner();
    if inner.w == 0 || inner.h == 0 {
        return Vec::new();
    }
    let mut output = format!("\x1b[{};{}H", inner.y + 1, inner.x + 1).into_bytes();
    let chunks: Vec<&[u8]> = payload.as_bytes().chunks(4096).collect();
    for (index, chunk) in chunks.iter().enumerate() {
        let more = usize::from(index + 1 < chunks.len());
        if index == 0 {
            output.extend_from_slice(
                format!(
                    "\x1b_Ga=T,f=24,s={width},v={height},c={},r={},q={},z=0,i={image_id},m={more};",
                    inner.w,
                    inner.h,
                    if require_ack() { 1 } else { 2 }
                )
                .as_bytes(),
            );
        } else {
            output.extend_from_slice(format!("\x1b_Gm={more};").as_bytes());
        }
        output.extend_from_slice(chunk);
        output.extend_from_slice(b"\x1b\\");
    }
    output
}

fn place_existing(rect: Rect, image_id: u32) -> Vec<u8> {
    let inner = rect.inner();
    if inner.w == 0 || inner.h == 0 {
        return Vec::new();
    }
    format!(
        "\x1b[{};{}H\x1b_Ga=p,i={image_id},c={},r={},z=0,q=2\x1b\\",
        inner.y + 1,
        inner.x + 1,
        inner.w,
        inner.h
    )
    .into_bytes()
}

fn require_ack() -> bool {
    env::var("HAPPYBIRTHDAY_KITTY_REQUIRE_ACK").is_ok_and(|value| value != "0")
}

pub struct KittyCache {
    key: Option<(u64, usize, usize)>,
    payload: String,
    sent_at: Option<Instant>,
    acknowledged: bool,
    require_ack: bool,
}

impl KittyCache {
    pub fn new() -> Self {
        Self {
            key: None,
            payload: String::new(),
            sent_at: None,
            acknowledged: false,
            require_ack: require_ack(),
        }
    }

    pub fn transfer(
        &mut self,
        cover_id: u64,
        rect: Rect,
        width: usize,
        height: usize,
        rgb: &[u8],
    ) -> Vec<u8> {
        let key = (cover_id, width, height);
        let new_image = self.key != Some(key);
        if new_image {
            self.payload = base64::engine::general_purpose::STANDARD.encode(rgb);
            self.key = Some(key);
            self.sent_at = Some(Instant::now());
            self.acknowledged = false;
            let mut output = delete_image();
            output.extend(place_encoded(rect, width, height, &self.payload, 7001));
            return output;
        }
        let mut output = delete_placement();
        output.extend(place_existing(rect, 7001));
        output
    }

    #[allow(dead_code)]
    pub fn ack(&mut self) {
        self.acknowledged = true;
    }

    pub fn invalidate(&mut self) {
        self.key = None;
        self.payload.clear();
        self.sent_at = None;
        self.acknowledged = false;
    }

    pub fn retransmit(&mut self, rect: Rect, width: usize, height: usize) -> Vec<u8> {
        self.sent_at = Some(Instant::now());
        place_encoded(rect, width, height, &self.payload, 7001)
    }

    pub fn should_fallback(&self, timeout: Duration) -> bool {
        self.require_ack
            && !self.acknowledged
            && self.sent_at.is_some_and(|sent| sent.elapsed() > timeout)
    }

    pub fn should_retransmit(&self, timeout: Duration) -> bool {
        self.require_ack
            && !self.acknowledged
            && self.sent_at.is_some_and(|sent| sent.elapsed() > timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_chunked_truecolor_image() {
        let image = vec![128; 8 * 8 * 3];
        let encoded = place(Rect::new(2, 2, 6, 5), 8, 8, &image);
        assert!(encoded.windows(5).any(|part| part == b"f=24,"));
        assert!(encoded.ends_with(b"\x1b\\"));
    }

    #[test]
    fn cache_transfers_pixels_once_and_reuses_image_for_moves() {
        let image = vec![64; 4 * 4 * 3];
        let mut cache = KittyCache::new();
        let first = cache.transfer(7, Rect::new(1, 1, 6, 4), 4, 4, &image);
        assert!(first.windows(5).any(|part| part == b"f=24,"));
        let moved = cache.transfer(7, Rect::new(8, 2, 6, 4), 4, 4, &image);
        assert!(!moved.windows(5).any(|part| part == b"f=24,"));
        assert!(moved.windows(3).any(|part| part == b"a=p"));
        assert!(moved.windows(7).any(|part| part == b"a=d,d=i"));
    }

    #[test]
    fn invalidation_forces_pixels_to_be_uploaded_again() {
        let image = vec![42; 2 * 2 * 3];
        let mut cache = KittyCache::new();
        let _ = cache.transfer(9, Rect::new(1, 1, 4, 3), 2, 2, &image);
        cache.invalidate();
        let restored = cache.transfer(9, Rect::new(1, 1, 4, 3), 2, 2, &image);
        assert!(restored.windows(5).any(|part| part == b"f=24,"));
    }
}
