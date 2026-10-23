use sha2::{Digest, Sha256};

pub fn visual_seed(text: &str, title: &str, artist: &str, bpm: f32) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher.update(title.as_bytes());
    hasher.update(artist.as_bytes());
    hasher.update(((bpm.max(20.0) * 10.0) as u32).to_le_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::visual_seed;

    #[test]
    fn lyric_and_bpm_produce_stable_distinct_seeds() {
        let a = visual_seed("乱れた呼吸", "一騎当千", "梅とら", 68.0);
        let b = visual_seed("乱れた呼吸", "一騎当千", "梅とら", 68.0);
        let c = visual_seed("鼓動急かす", "一騎当千", "梅とら", 68.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
