// What this build of the SMB provider can do, for the Services status panel: each login method and
// protection says whether it works here and, when it does not, why.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Whether one thing works in this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Supported,
    /// It does not, and why, as the Services panel words it.
    Unsupported {
        reason: &'static str,
    },
}

impl Support {
    pub fn is_supported(&self) -> bool {
        matches!(self, Support::Supported)
    }
}

/// Where the SMB protocol is spoken: by Waypoint, or by the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// The pure Rust `smb2` client (Linux).
    Client,
    /// Windows' own client, through UNC paths and `WNetAddConnection2` (unverified at runtime).
    Os,
    /// Neither: the crate was built without its `client` feature.
    None,
}

/// What an SMB location supports in this build (A99).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Availability {
    pub engine: Engine,
    /// A user name and password, with an optional domain.
    pub ntlm: Support,
    /// A domain account through the user's Kerberos ticket (single sign-on).
    pub kerberos: Support,
    /// Signed messages, which a server may require.
    pub signing: Support,
    /// Encrypted shares (SMB 3).
    pub encryption: Support,
    /// `smb://host/` lists the server's shares.
    pub share_browser: Support,
}

const NO_CLIENT: Support = Support::Unsupported {
    reason: "this build of Waypoint has no SMB client",
};

/// What this build can do.
pub fn availability() -> Availability {
    if cfg!(windows) {
        // The OS client does all of it, as Explorer does.
        return Availability {
            engine: Engine::Os,
            ntlm: Support::Supported,
            kerberos: Support::Supported,
            signing: Support::Supported,
            encryption: Support::Supported,
            share_browser: Support::Supported,
        };
    }
    if cfg!(feature = "client") {
        return Availability {
            engine: Engine::Client,
            ntlm: Support::Supported,
            kerberos: Support::Unsupported {
                reason: "the SMB library Waypoint uses has no Kerberos login yet; use a user name and password",
            },
            signing: Support::Supported,
            encryption: Support::Unsupported {
                reason: "the SMB library could not negotiate encryption with Samba 4.24 in testing, so a server or share that requires it cannot be opened",
            },
            share_browser: Support::Supported,
        };
    }
    Availability {
        engine: Engine::None,
        ntlm: NO_CLIENT,
        kerberos: NO_CLIENT,
        signing: NO_CLIENT,
        encryption: NO_CLIENT,
        share_browser: NO_CLIENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsupported_method_says_why() {
        let what = availability();
        if cfg!(all(not(windows), feature = "client")) {
            assert_eq!(what.engine, Engine::Client);
            assert!(what.ntlm.is_supported());
            let Support::Unsupported { reason } = what.kerberos else {
                panic!("Kerberos is not offered by the library");
            };
            assert!(reason.contains("Kerberos"));
        }
    }
}
