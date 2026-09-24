use serde::{Deserialize, Serialize};

/// A size in bytes. Stored as `u64` internally (sectors/bytes on disk never
/// need to be negative and rarely approach `u64::MAX`), but serialized as a
/// string over IPC: JavaScript's `number` cannot represent integers above
/// 2^53-1 without loss, and real disks comfortably exceed that (a 16 TiB
/// disk is already past it). The frontend parses it with `BigInt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteSize(#[serde(with = "as_string")] pub u64);

impl ByteSize {
    pub const ZERO: ByteSize = ByteSize(0);

    pub fn bytes(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, other: ByteSize) -> Option<ByteSize> {
        self.0.checked_add(other.0).map(ByteSize)
    }

    pub fn checked_sub(self, other: ByteSize) -> Option<ByteSize> {
        self.0.checked_sub(other.0).map(ByteSize)
    }
}

impl From<u64> for ByteSize {
    fn from(v: u64) -> Self {
        ByteSize(v)
    }
}

mod as_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let s = String::deserialize(d)?;
        s.parse::<u64>().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_string_to_avoid_js_number_precision_loss() {
        let size = ByteSize(16_000_000_000_000); // 16 TB, well past 2^53-1
        let json = serde_json::to_string(&size).unwrap();
        assert_eq!(json, "\"16000000000000\"");
        let back: ByteSize = serde_json::from_str(&json).unwrap();
        assert_eq!(back, size);
    }

    #[test]
    fn checked_sub_none_on_underflow() {
        assert_eq!(ByteSize(5).checked_sub(ByteSize(10)), None);
    }
}
