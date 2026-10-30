//! Wi-Fi presentation and typed actions. Credentials remain with NetworkManager.
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

/// Bounded owned entry buffer. Debug is redacted; this type cannot be serialized.
/// Copies in input libraries, D-Bus marshalling and the OS are outside this scope.
#[derive(PartialEq, Eq)]
pub struct Secret(Zeroizing<String>);
impl Default for Secret {
    fn default() -> Self {
        Self::new(String::new())
    }
}
impl Secret {
    pub fn new(mut value: String) -> Self {
        let mut buffer = String::with_capacity(128);
        if value.len() <= 128 && !value.chars().any(char::is_control) {
            buffer.push_str(&value);
        }
        value.zeroize();
        Self(Zeroizing::new(buffer))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn append(&mut self, value: &Self) {
        if self.0.len() + value.0.len() <= 128 {
            self.0.push_str(&value.0);
        }
    }
    pub fn backspace(&mut self) {
        self.0.pop();
    }
}
impl Clone for Secret {
    fn clone(&self) -> Self {
        Self::new(self.expose().to_owned())
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}
impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value = String::deserialize(deserializer)?;
        if value.len() > 128 || value.chars().any(char::is_control) {
            value.zeroize();
            return Err(serde::de::Error::custom("Invalid password entry"));
        }
        Ok(Self::new(value))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum PasswordKind {
    Open,
    Psk,
    Sae,
}
impl PasswordKind {
    pub fn accepts(self, password: &Secret) -> bool {
        let text = password.expose();
        match self {
            Self::Open => text.is_empty(),
            Self::Psk => {
                (8..=63).contains(&text.len()) && text.is_ascii()
                    || text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
            }
            Self::Sae => !text.is_empty() && text.len() <= 128,
        }
    }
    pub fn guidance(self) -> &'static str {
        match self {
            Self::Open => "This network has no password. Traffic may be visible to others.",
            Self::Psk => "Use 8–63 ASCII characters or 64 hexadecimal digits.",
            Self::Sae => "Enter the WPA3 password (up to 128 bytes).",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SecretPrompt {
    pub id: u64,
    pub label: String,
    pub kind: PasswordKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputTarget {
    New(u64),
    Agent(u64),
}
#[derive(Clone, Debug)]
pub struct Input {
    pub target: InputTarget,
    pub label: String,
    pub kind: PasswordKind,
    pub password: Secret,
    pub remember: bool,
    pub keyboard: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Permission {
    Yes,
    Authentication,
    #[default]
    No,
}
impl Permission {
    pub fn from_nm(value: Option<&String>) -> Self {
        match value.map(String::as_str) {
            Some("yes") => Self::Yes,
            Some("auth") => Self::Authentication,
            _ => Self::No,
        }
    }
    pub fn allowed(self) -> bool {
        self == Self::Yes
    }
    pub fn can_request(self, can_prompt: bool) -> bool {
        self.allowed() || self == Self::Authentication && can_prompt
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Security {
    Open,
    Personal,
    Enterprise,
    Unsupported,
}
impl Security {
    pub fn from_flags(flags: u32, wpa: u32, rsn: u32) -> Self {
        let security = wpa | rsn;
        if security & (0x100 | 0x400) != 0 {
            Self::Personal
        } else if security & 0x200 != 0 {
            Self::Enterprise
        } else if flags & 1 == 0 && security == 0 {
            Self::Open
        } else {
            Self::Unsupported
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Personal => "Personal security",
            Self::Enterprise => "Enterprise",
            Self::Unsupported => "Unsupported security",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum State {
    #[default]
    Unknown,
    Unavailable,
    Disconnected,
    Connecting,
    CredentialsRequired,
    Connected,
    Disconnecting,
    Failed,
}
impl State {
    pub fn from_nm(value: u32) -> Self {
        match value {
            10 | 20 => Self::Unavailable,
            30 => Self::Disconnected,
            40 | 50 | 70 | 80 | 90 => Self::Connecting,
            60 => Self::CredentialsRequired,
            100 => Self::Connected,
            110 => Self::Disconnecting,
            120 => Self::Failed,
            _ => Self::Unknown,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "State unknown",
            Self::Unavailable => "Wi-Fi unavailable",
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::CredentialsRequired => "Credentials required",
            Self::Connected => "Connected",
            Self::Disconnecting => "Disconnecting",
            Self::Failed => "Connection failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub id: u64,
    pub label: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Network {
    pub id: u64,
    pub label: String,
    pub strength: u8,
    pub security: Security,
    pub connected: bool,
    pub profiles: Vec<Profile>,
    pub password_kind: Option<PasswordKind>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Pending {
    Scan,
    Radio(bool),
    Connect(u64),
    Disconnect,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub generation: u64,
    pub epoch: u64,
    pub enabled: bool,
    pub available: bool,
    pub radio: bool,
    pub hardware: bool,
    pub state: State,
    pub state_reason: u32,
    pub connectivity: u32,
    pub scan_permission: Permission,
    pub control_permission: Permission,
    pub radio_permission: Permission,
    pub last_scan: i64,
    pub networks: Vec<Network>,
    pub active_profile: Option<u64>,
    pub busy: bool,
    pub pending: Option<Pending>,
    pub error: Option<String>,
    pub action_error: Option<String>,
    pub create_permission: Permission,
    pub agent_available: bool,
    pub secret_prompt: Option<SecretPrompt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum UiAction {
    Scan,
    Radio {
        enabled: bool,
    },
    Connect {
        network: u64,
        profile: u64,
    },
    Disconnect,
    Create {
        network: u64,
        password: Secret,
        remember: bool,
    },
    Secrets {
        request: u64,
        password: Secret,
    },
    CancelSecrets {
        request: u64,
    },
}

pub fn ssid_label(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text
            .chars()
            .flat_map(|c| {
                if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
                    c.escape_unicode().collect::<Vec<_>>()
                } else {
                    vec![c]
                }
            })
            .collect(),
        Err(_) => bytes.iter().map(|b| format!("\\x{b:02x}")).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_entry_is_bounded_redacted_and_validated_for_its_protocol() {
        let mut secret = Secret::new("password".into());
        assert!(PasswordKind::Psk.accepts(&secret));
        assert!(!format!("{secret:?}").contains("password"));
        secret.append(&Secret::new("é".into()));
        assert!(!PasswordKind::Psk.accepts(&secret));
        assert!(PasswordKind::Sae.accepts(&secret));
        secret.backspace();
        assert_eq!(secret.expose(), "password");
        assert!(PasswordKind::Psk.accepts(&Secret::new("f".repeat(64))));
        assert!(!PasswordKind::Psk.accepts(&Secret::new("z".repeat(64))));
        secret.append(&Secret::new("z".repeat(128)));
        assert_eq!(secret.expose(), "password");
        assert!(Secret::new("x".repeat(129)).expose().is_empty());
        assert!(PasswordKind::Open.accepts(&Secret::default()));
    }
    #[test]
    fn network_labels_and_security_do_not_merge_different_networks() {
        assert_eq!(ssid_label("café".as_bytes()), "café");
        assert_eq!(ssid_label(&[255, 0]), "\\xff\\x00");
        assert_eq!(ssid_label(b"a\nb"), "a\\u{a}b");
        assert_eq!(Security::from_flags(0, 0, 0), Security::Open);
        assert_eq!(Security::from_flags(1, 0, 0), Security::Unsupported);
        assert_eq!(Security::from_flags(1, 0, 0x100), Security::Personal);
        assert_eq!(Security::from_flags(1, 0, 0x200), Security::Enterprise);
        assert_eq!(
            Permission::from_nm(Some(&"auth".into())),
            Permission::Authentication
        );
        assert!(!Permission::Authentication.allowed());
        assert_eq!(State::from_nm(70), State::Connecting);
        assert_eq!(State::from_nm(100), State::Connected);
        assert_eq!(State::from_nm(60), State::CredentialsRequired);
    }
}
