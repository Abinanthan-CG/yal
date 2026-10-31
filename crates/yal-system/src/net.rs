//! The one network door.
//!
//! Every fetch here sits behind an explicit act — the current caller is
//! the exchange-rate update (`yal rates-update`, or the palette's act),
//! which runs when asked, never at startup, never on a schedule. The
//! launcher works with the machine unplugged; the network earns its
//! place one request at a time.

use std::io::Read;
use std::time::Duration;

/// The Frankfurter `latest` endpoint: one GET answers every currency
/// against the US dollar, keyed by nothing, published by the European
/// Central Bank's daily fixing.
pub const DEFAULT_RATES_URL: &str = "https://api.frankfurter.dev/v1/latest?base=USD";

/// A fetch that failed, or answered out of bounds.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("the request did not answer: {0}")]
    Request(#[from] reqwest::Error),

    #[error("the reply was larger than the {0}-byte cap")]
    TooLarge(usize),

    #[error("reading the reply failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("the body was not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Fetch and parse a JSON document, with a hard timeout and a hard size
/// cap. The cap is counted while reading, so a reply far larger than
/// promised cannot walk the process into memory it does not have.
pub fn fetch_json(
    url: &str,
    timeout: Duration,
    max_bytes: usize,
) -> Result<serde_json::Value, NetError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()?;
    let response = client.get(url).send()?.error_for_status()?;

    // Reading is capped one byte past the limit, so crossing the cap
    // proves itself instead of being met by luck.
    let mut body = Vec::with_capacity(64 * 1024);
    let read = response
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut body)?;
    if read > max_bytes {
        return Err(NetError::TooLarge(max_bytes));
    }

    Ok(serde_json::from_slice(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_connection_is_an_error_not_a_hang() {
        // Port 9 (discard) on localhost: nothing listens there. The
        // fetch refuses quickly and reports it; no network needed.
        let started = std::time::Instant::now();
        let result = fetch_json(
            "http://127.0.0.1:9/altogether-unlikely",
            Duration::from_millis(2_000),
            1024,
        );
        assert!(result.is_err(), "nothing listens on the discard port");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "a refused connection answers fast, not on the timeout"
        );
    }
}
