//! Content-minimal notification delivery boundary.
//!
//! APNs and FCM implementations must construct their fixed reconnect payload
//! internally. Relay callers cannot provide titles, bodies, entity links,
//! counts, or event identifiers because those fields do not exist here.

use async_trait::async_trait;
use thiserror::Error;

/// Push transport family selected by an installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MkPushProvider {
    /// Apple Push Notification service.
    Apns,
    /// Firebase Cloud Messaging.
    Fcm,
}

/// Content-free request to wake one opaque installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MkReconnectWake {
    /// Provider selected during installation enrollment.
    pub provider: MkPushProvider,
    /// Opaque installation identifier; never a platform token in logs.
    pub installation_id: String,
}

/// Notification transport error without secret-bearing provider payloads.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum MkPushError {
    /// Opaque installation identifier is malformed.
    #[error("invalid installation identifier")]
    InvalidInstallation,
    /// Provider rejected or failed the reconnect wake.
    #[error("push provider unavailable")]
    ProviderUnavailable,
}

/// Transport that can send only the fixed reconnect signal.
#[async_trait]
pub trait MkPushTransport: Send + Sync {
    /// Deliver a content-free reconnect wake.
    async fn send_reconnect(&self, wake: &MkReconnectWake) -> Result<(), MkPushError>;
}

/// Validate the non-secret request boundary before any provider call.
pub fn validate_wake(wake: &MkReconnectWake) -> Result<(), MkPushError> {
    if wake.installation_id.len() < 8
        || wake.installation_id.len() > 128
        || !wake
            .installation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(MkPushError::InvalidInstallation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeTransport {
        wakes: Mutex<Vec<MkReconnectWake>>,
    }

    #[async_trait]
    impl MkPushTransport for FakeTransport {
        async fn send_reconnect(&self, wake: &MkReconnectWake) -> Result<(), MkPushError> {
            validate_wake(wake)?;
            self.wakes
                .lock()
                .map_err(|_| MkPushError::ProviderUnavailable)?
                .push(wake.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn fake_apns_and_fcm_receive_only_reconnect_contract() {
        let transport = FakeTransport::default();
        for provider in [MkPushProvider::Apns, MkPushProvider::Fcm] {
            transport
                .send_reconnect(&MkReconnectWake {
                    provider,
                    installation_id: format!("installation-{provider:?}"),
                })
                .await
                .expect("fake wake accepted");
        }
        let wakes = transport.wakes.lock().expect("fake wake lock");
        assert_eq!(wakes.len(), 2);
        assert_eq!(wakes[0].provider, MkPushProvider::Apns);
        assert_eq!(wakes[1].provider, MkPushProvider::Fcm);
    }

    #[tokio::test]
    async fn malformed_installation_never_reaches_transport_queue() {
        let transport = FakeTransport::default();
        let result = transport
            .send_reconnect(&MkReconnectWake {
                provider: MkPushProvider::Fcm,
                installation_id: "raw token with spaces".into(),
            })
            .await;
        assert_eq!(result, Err(MkPushError::InvalidInstallation));
        assert!(transport.wakes.lock().expect("fake wake lock").is_empty());
    }
}
