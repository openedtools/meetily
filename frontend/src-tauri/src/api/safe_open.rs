//! Opening links outside the app, safely.
//!
//! A link can come from untrusted text (for example a summary written by a language model that was
//! fed hostile content). It must never be handed to a shell, and only ordinary web and mail links
//! may be opened: other schemes (`file:`, `ms-msdt:`, custom protocol handlers) can launch programs.

/// Returns the link in normalised form if it is an http, https or mailto link, otherwise an error.
pub(crate) fn validated_external_url(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw.trim()).map_err(|_| "That is not a valid link".to_string())?;
    match parsed.scheme() {
        "http" | "https" | "mailto" => Ok(parsed.to_string()),
        other => Err(format!("Links of type '{other}:' are not opened for safety")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_web_and_mail_links_are_allowed() {
        assert_eq!(validated_external_url("https://example.com/a?x=1&y=2").unwrap(), "https://example.com/a?x=1&y=2");
        assert_eq!(validated_external_url("  http://example.com  ").unwrap(), "http://example.com/");
        assert!(validated_external_url("mailto:someone@example.com").is_ok());
    }

    #[test]
    fn links_that_could_launch_programs_are_refused() {
        for hostile in [
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            "ms-msdt:/id PCWDiagnostic /skip force /param \"IT_RebrowseForFile=?\"",
            "search-ms:query=a&crumb=location:\\\\evil\\share",
            "vbscript:msgbox(1)",
            "data:text/html,<script>alert(1)</script>",
            "ssh://host",
            "calc.exe",
            "C:\\Windows\\System32\\cmd.exe",
            "\\\\evil\\share\\payload.exe",
            "",
        ] {
            assert!(validated_external_url(hostile).is_err(), "{hostile:?} must be refused");
        }
    }

    #[test]
    fn shell_metacharacters_stay_inside_the_link() {
        // Opened without a shell these are just URL characters; the old `cmd /C start` treated `&` as a command separator
        let url = validated_external_url("https://example.com/&calc.exe").unwrap();
        assert!(url.starts_with("https://example.com/"));
        assert!(validated_external_url("https://example.com/\"&calc&\"").is_ok());
    }
}
