/// A single satisfiable byte range, inclusive on both ends.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

impl ByteRange {
    pub fn len(&self) -> u64 {
        self.end - self.start + 1
    }
}

/// Parses a `Range` header against a resource of `len` bytes.
/// Returns `None` for absent, malformed, multi-part or unsatisfiable ranges —
/// the caller then serves the full body with 200, which is what the spec allows
/// when a range cannot be honoured.
pub fn parse_range(header: &str, len: u64) -> Option<ByteRange> {
    let spec = header.strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;

    let (start, end) = match (a.trim(), b.trim()) {
        ("", "") => return None,
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            if n == 0 {
                return None;
            }
            (len.saturating_sub(n), len - 1)
        }
        (start, "") => {
            let s: u64 = start.parse().ok()?;
            if s >= len {
                return None;
            }
            (s, len - 1)
        }
        (start, end) => {
            let s: u64 = start.parse().ok()?;
            let e: u64 = end.parse().ok()?;
            if s > e || s >= len {
                return None;
            }
            (s, e.min(len - 1))
        }
    };

    Some(ByteRange { start, end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_range() {
        assert_eq!(parse_range("bytes=0-499", 1000), Some(ByteRange { start: 0, end: 499 }));
    }

    #[test]
    fn open_ended_range() {
        assert_eq!(parse_range("bytes=500-", 1000), Some(ByteRange { start: 500, end: 999 }));
    }

    #[test]
    fn suffix_range() {
        assert_eq!(parse_range("bytes=-100", 1000), Some(ByteRange { start: 900, end: 999 }));
    }

    #[test]
    fn suffix_longer_than_file_is_clamped() {
        assert_eq!(parse_range("bytes=-5000", 1000), Some(ByteRange { start: 0, end: 999 }));
    }

    #[test]
    fn end_beyond_file_is_clamped() {
        assert_eq!(parse_range("bytes=900-99999", 1000), Some(ByteRange { start: 900, end: 999 }));
    }

    #[test]
    fn rejects_unsatisfiable_and_garbage() {
        assert_eq!(parse_range("bytes=1000-1100", 1000), None);
        assert_eq!(parse_range("bytes=500-499", 1000), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=-0", 1000), None);
        assert_eq!(parse_range("bytes=", 1000), None);
        assert_eq!(parse_range("bytes=0-10", 0), None);
    }

    #[test]
    fn length_is_inclusive() {
        let r = parse_range("bytes=10-19", 100).unwrap();
        assert_eq!(r.len(), 10);
    }
}
