use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhatsminerLuciAuth {
    pub username: String,
    pub password: String,
}

/// WhatsMiner-only fetch options. Never pass to other drivers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhatsminerFetchOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luci_auth: Option<WhatsminerLuciAuth>,
    /// Skip slow LuCI chip dump and non-essential API calls (poll / fast read).
    #[serde(default)]
    pub fast_poll: bool,
    /// LuCI chip matrix — independent of `fast_poll` (read path).
    #[serde(default)]
    pub fetch_chips: bool,
}

impl WhatsminerFetchOptions {
    pub fn fast_read() -> Self {
        Self {
            fast_poll: true,
            fetch_chips: false,
            ..Default::default()
        }
    }

    /// 4028 telemetry only; LuCI chip dump when credentials are available.
    pub fn read_once(luci_auth: Option<WhatsminerLuciAuth>) -> Self {
        Self {
            fast_poll: true,
            fetch_chips: true,
            luci_auth,
            ..Default::default()
        }
    }

    pub fn luci_credential_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        let mut push = |user: &str, pass: &str| {
            if user.is_empty() {
                return;
            }
            if pairs.iter().any(|(u, p)| u == user && p == pass) {
                return;
            }
            pairs.push((user.to_string(), pass.to_string()));
        };

        if let Some(auth) = &self.luci_auth {
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
        let options = WhatsminerFetchOptions::default();
        let pairs = options.luci_credential_pairs();
        assert_eq!(pairs, vec![("admin".to_string(), "admin".to_string())]);
    }

    #[test]
    fn custom_credentials_skip_defaults() {
        let options = WhatsminerFetchOptions {
            luci_auth: Some(WhatsminerLuciAuth {
                username: "root".into(),
                password: "root".into(),
            }),
            fast_poll: false,
            fetch_chips: false,
        };
        let pairs = options.luci_credential_pairs();
        assert_eq!(pairs, vec![("root".to_string(), "root".to_string())]);
    }
}
