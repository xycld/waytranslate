//! HMAC-SHA256 / SHA256 helpers shared by tencent.rs and volcengine.rs.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, digest::KeyInit};

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

pub fn hmac_sha256_hex(key: &[u8], msg: &[u8]) -> String {
    hex::encode(hmac_sha256(key, msg))
}

/// UTC timestamp helpers for signing (no chrono dependency).
pub struct Timestamp {
    pub epoch: i64,
    pub date: String,     // YYYYMMDD
    pub datetime: String, // YYYYMMDDTHHMMSSZ (with literal T/Z)
}

pub fn now_utc() -> Timestamp {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = epoch.div_euclid(86400);
    let secs = epoch.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    Timestamp {
        epoch,
        date: format!("{y:04}{m:02}{d:02}"),
        datetime: format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z"),
    }
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    #[test]
    fn civil_epoch() {
        assert_eq!(super::civil_from_days(0), (1970, 1, 1));
        let t = super::now_utc();
        assert!(t.date.len() == 8);
    }

    #[test]
    fn hmac_known_vector() {
        // RFC 4231 test case 1
        let key = [0x0bu8; 20];
        let out = super::hmac_sha256_hex(&key, b"Hi There");
        assert_eq!(
            out,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
}
