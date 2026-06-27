use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhatsminerLuciAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FetchOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whatsminer_luci: Option<WhatsminerLuciAuth>,
    /// Skip slow LuCI chip dump and non-essential API calls (poll loop).
    #[serde(default)]
    pub fast_poll: bool,
}

impl FetchOptions {
    pub fn luci_credential_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        let mut push = |user: &str, pass: &str| {
            if user.is_empty() {
                return;
            }
            if pairs
                .iter()
                .any(|(u, p)| u == user && p == pass)
            {
                return;
            }
            pairs.push((user.to_string(), pass.to_string()));
        };

        if let Some(auth) = &self.whatsminer_luci {
            push(&auth.username, &auth.password);
            return pairs;
        }
        push("admin", "admin");
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_credentials_are_admin_admin() {
        let options = FetchOptions::default();
        let pairs = options.luci_credential_pairs();
        assert_eq!(pairs, vec![("admin".to_string(), "admin".to_string())]);
    }

    #[test]
    fn custom_credentials_skip_defaults() {
        let options = FetchOptions {
            whatsminer_luci: Some(WhatsminerLuciAuth {
                username: "root".into(),
                password: "root".into(),
            }),
            fast_poll: false,
        };
        let pairs = options.luci_credential_pairs();
        assert_eq!(pairs, vec![("root".to_string(), "root".to_string())]);
    }

    #[test]
    fn prefers_custom_credentials_first() {
        let options = FetchOptions {
            whatsminer_luci: Some(WhatsminerLuciAuth {
                username: "miner".into(),
                password: "secret".into(),
            }),
            fast_poll: false,
        };
        let pairs = options.luci_credential_pairs();
        assert_eq!(pairs[0], ("miner".to_string(), "secret".to_string()));
    }
}
