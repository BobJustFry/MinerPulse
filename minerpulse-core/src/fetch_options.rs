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
        }
        push("root", "root");
        push("admin", "admin");
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_default_credentials() {
        let options = FetchOptions {
            whatsminer_luci: Some(WhatsminerLuciAuth {
                username: "root".into(),
                password: "root".into(),
            }),
        };
        let pairs = options.luci_credential_pairs();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("root".to_string(), "root".to_string()));
        assert_eq!(pairs[1], ("admin".to_string(), "admin".to_string()));
    }

    #[test]
    fn prefers_custom_credentials_first() {
        let options = FetchOptions {
            whatsminer_luci: Some(WhatsminerLuciAuth {
                username: "miner".into(),
                password: "secret".into(),
            }),
        };
        let pairs = options.luci_credential_pairs();
        assert_eq!(pairs[0], ("miner".to_string(), "secret".to_string()));
    }
}
