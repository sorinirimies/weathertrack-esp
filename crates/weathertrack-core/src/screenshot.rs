//! Pure parts of the firmware's screenshot endpoint (feature `shots` of the P4 firmware):
//! which page a request asks for, and the BMP header of the frame buffer it streams.

use crate::web::parse_query;

/// A page change to make before the screen is captured.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShotRequest {
    pub tab: Option<usize>,
    pub search: Option<String>,
    /// Terminal UI theme index (the stored theme otherwise).
    pub theme: Option<usize>,
}

impl ShotRequest {
    /// `/screenshot.bmp?tab=4&search=Stuttgart` is tab 4 with that search typed and run.
    pub fn parse(uri: &str) -> Self {
        let q = parse_query(uri.split_once('?').map_or("", |(_, q)| q));
        let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        Self {
            tab: get("tab").and_then(|v| v.parse().ok()),
            search: get("search").filter(|s| !s.trim().is_empty()),
            theme: get("theme").and_then(|v| v.parse().ok()),
        }
    }

    /// The main loop has to show something before the capture.
    pub fn changes_page(&self) -> bool {
        self.tab.is_some() || self.search.is_some() || self.theme.is_some()
    }
}

/// Size of the BMP header written by [`bmp_rgb565_header`].
pub const BMP_HEADER_LEN: usize = 66;

/// Total size of a `width` x `height` 16-bit BMP (header and bottom-up rows).
pub fn bmp_rgb565_len(width: usize, height: usize) -> usize {
    BMP_HEADER_LEN + width * height * 2
}

/// Header of a 16-bit RGB565 BMP (`BI_BITFIELDS`); the rows follow, bottom row first.
pub fn bmp_rgb565_header(width: usize, height: usize) -> Vec<u8> {
    let data = (width * height * 2) as u32;
    let mut h = Vec::with_capacity(BMP_HEADER_LEN);
    h.extend_from_slice(b"BM");
    h.extend_from_slice(&(BMP_HEADER_LEN as u32 + data).to_le_bytes());
    h.extend_from_slice(&[0; 4]);
    h.extend_from_slice(&(BMP_HEADER_LEN as u32).to_le_bytes()); // where the pixels start
    h.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
    h.extend_from_slice(&(width as i32).to_le_bytes());
    h.extend_from_slice(&(height as i32).to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes()); // planes
    h.extend_from_slice(&16u16.to_le_bytes()); // bits per pixel
    h.extend_from_slice(&3u32.to_le_bytes()); // BI_BITFIELDS
    h.extend_from_slice(&data.to_le_bytes());
    h.extend_from_slice(&[0; 16]); // resolution and palette
    for mask in [0xF800u32, 0x07E0, 0x001F] {
        h.extend_from_slice(&mask.to_le_bytes());
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_names_a_page_and_a_search() {
        assert_eq!(
            ShotRequest::parse("/screenshot.bmp"),
            ShotRequest::default()
        );
        assert!(!ShotRequest::parse("/screenshot.bmp").changes_page());
        let r = ShotRequest::parse("/screenshot.bmp?tab=4&search=Bad%20Rappenau");
        assert_eq!(
            (r.tab, r.search.as_deref()),
            (Some(4), Some("Bad Rappenau"))
        );
        assert!(r.changes_page());
        assert_eq!(
            ShotRequest::parse("/screenshot.bmp?tab=x&search=%20").tab,
            None
        );
        assert_eq!(
            ShotRequest::parse("/screenshot.bmp?search=%20").search,
            None,
            "blank is no search"
        );
    }

    #[test]
    fn the_header_describes_the_frame_buffer() {
        let h = bmp_rgb565_header(1024, 600);
        assert_eq!(h.len(), BMP_HEADER_LEN);
        assert_eq!(&h[..2], b"BM");
        let u32_at = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
        assert_eq!(u32_at(2) as usize, bmp_rgb565_len(1024, 600), "file size");
        assert_eq!(
            u32_at(10) as usize,
            BMP_HEADER_LEN,
            "pixels start after the header"
        );
        assert_eq!((u32_at(18), u32_at(22)), (1024, 600));
        assert_eq!(u16::from_le_bytes([h[28], h[29]]), 16);
        assert_eq!(u32_at(30), 3, "bitfields");
        assert_eq!(
            (u32_at(54), u32_at(58), u32_at(62)),
            (0xF800, 0x07E0, 0x001F),
            "RGB565 masks"
        );
        assert_eq!(bmp_rgb565_len(1024, 600), 1_228_866);
    }
}
