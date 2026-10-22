//! Toast notifications, used for timers and other feedback.
//!
//! An unpackaged program may show toasts through a registered user model
//! identifier. This one uses its own identifier; where a host refuses the
//! toast, the failure is reported back to the caller instead of swallowed,
//! and the palette raises its own banner as a fallback.

/// A failure while sending a toast.
#[derive(Debug, thiserror::Error)]
pub enum NotifyError {
    #[error("the notification could not be shown: {0}")]
    Failed(String),
}

/// The user model identifier the launcher's toasts are sent from.
#[cfg(windows)]
pub const APPLICATION_ID: &str = "local.legacy-store";

/// Show a toast with a title and a body.
#[cfg(windows)]
pub fn toast(title: &str, body: &str) -> Result<(), NotifyError> {
    toast_with(APPLICATION_ID, title, body)
}

/// Show a toast from a named user model identifier.
///
/// For an unpackaged program Windows decides by the identifier whether a
/// banner may be shown; only identifiers the shell knows are trusted. A
/// caller may pass a trusted one — the id of an installed application —
/// when its own is not registered yet.
#[cfg(windows)]
pub fn toast_with(
    application_id: &str,
    title: &str,
    body: &str,
) -> Result<(), NotifyError> {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    let template = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text></binding></visual></toast>"#,
        title = escape_xml(title),
        body = escape_xml(body),
    );

    let xml = XmlDocument::new().map_err(|err| NotifyError::Failed(err.to_string()))?;
    xml.LoadXml(&HSTRING::from(template.as_str()))
        .map_err(|err| NotifyError::Failed(err.to_string()))?;

    let notification = ToastNotification::CreateToastNotification(&xml)
        .map_err(|err| NotifyError::Failed(err.to_string()))?;

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
        application_id,
    ))
    .map_err(|err| NotifyError::Failed(err.to_string()))?;

    notifier
        .Show(&notification)
        .map_err(|err| NotifyError::Failed(err.to_string()))
}

/// Show a toast on a host without the notification system.
#[cfg(not(windows))]
pub fn toast(_title: &str, _body: &str) -> Result<(), NotifyError> {
    Ok(())
}

/// Escape the text that goes into a toast so it cannot become markup.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_markup_characters() {
        let escaped = escape_xml("a<b>&\"c\"</b>");
        assert_eq!(escaped, "a&lt;b&gt;&amp;&quot;c&quot;&lt;/b&gt;");
    }

    #[test]
    fn an_injection_attempt_is_neutralised() {
        let escaped = escape_xml("</text><toast>");
        assert!(!escaped.contains("</text>"));
    }
}
