//! Bounded controller messages and public-origin validation.
use std::net::IpAddr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum RunnerCommand {
    Navigate { tab_id: String, url: String },
    CloseTab { tab_id: String },
    Shutdown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum RunnerEvent {
    TabNavigated {
        tab_id: String,
        origin: String,
        title: String,
    },
    DownloadRequested {
        download_id: String,
        url: String,
        suggested_filename: String,
    },
    Error {
        tab_id: Option<String>,
        message: String,
    },
}

/// Return an ASCII HTTPS origin only for a public destination.
pub fn sanitize_origin(input: &str) -> Result<String, String> {
    if input.len() > 4096 {
        return Err("Address is too long".into());
    }
    let url = url::Url::parse(input).map_err(|_| "Invalid address")?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("A public HTTPS address is required".into());
    }
    let host = url
        .host_str()
        .ok_or("Missing host")?
        .trim_matches(['[', ']'])
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if let Ok(ip) = host.parse::<IpAddr>() {
        let blocked = match ip {
            IpAddr::V4(v) => {
                v.is_private()
                    || v.is_loopback()
                    || v.is_link_local()
                    || v.is_multicast()
                    || v.is_unspecified()
                    || v.is_broadcast()
                    || v.is_documentation()
                    || v.octets()[0] == 0
                    || v.octets()[0] >= 240
                    || (v.octets()[0] == 100 && (64..=127).contains(&v.octets()[1]))
                    || (v.octets()[0] == 198 && (18..=19).contains(&v.octets()[1]))
            }
            IpAddr::V6(v) => {
                v.is_loopback()
                    || v.is_unspecified()
                    || v.is_multicast()
                    || (v.segments()[0] & 0xfe00) == 0xfc00
                    || (v.segments()[0] & 0xffc0) == 0xfe80
                    || v.to_ipv4_mapped().is_some()
            }
        };
        if blocked {
            return Err("Private or local address".into());
        }
    } else if !host.contains('.')
        || [
            ".localhost",
            ".local",
            ".lan",
            ".internal",
            ".home",
            ".home.arpa",
            ".test",
            ".invalid",
        ]
        .iter()
        .any(|suffix| host.ends_with(suffix))
    {
        return Err("Local address".into());
    }
    Ok(url.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_and_events_round_trip() {
        for command in [
            RunnerCommand::Navigate {
                tab_id: "tab".into(),
                url: "https://example.com".into(),
            },
            RunnerCommand::CloseTab {
                tab_id: "tab".into(),
            },
            RunnerCommand::Shutdown,
        ] {
            assert_eq!(
                serde_json::from_str::<RunnerCommand>(&serde_json::to_string(&command).unwrap())
                    .unwrap(),
                command
            );
        }
        for event in [
            RunnerEvent::TabNavigated {
                tab_id: "tab".into(),
                origin: "https://example.com".into(),
                title: "Example".into(),
            },
            RunnerEvent::DownloadRequested {
                download_id: "id".into(),
                url: "https://example.com/file".into(),
                suggested_filename: "file".into(),
            },
            RunnerEvent::Error {
                tab_id: None,
                message: "error".into(),
            },
        ] {
            assert_eq!(
                serde_json::from_str::<RunnerEvent>(&serde_json::to_string(&event).unwrap())
                    .unwrap(),
                event
            );
        }
    }
    #[test]
    fn only_public_https_origins() {
        assert_eq!(
            sanitize_origin("https://ExAmPlE.com/path").unwrap(),
            "https://example.com"
        );
        for address in [
            "http://example.com",
            "https://127.0.0.1",
            "https://10.0.0.1",
            "https://192.168.1.1",
            "https://[::1]",
            "https://[fe80::1]",
            "https://printer.local",
            "https://hub.lan",
            "https://user:pass@example.com",
        ] {
            assert!(sanitize_origin(address).is_err(), "{address}");
        }
    }
}
