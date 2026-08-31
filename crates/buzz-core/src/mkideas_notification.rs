//! Actionable MK Ideas notification protocol.
//!
//! Preferences and deduplication are server-side policy, while push transports
//! receive only a content-free reconnect wake. Engagement-oriented classes are
//! intentionally absent from this closed vocabulary.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

/// Actionable notification classes supported by MK Ideas Buzz.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MkNotificationClass {
    /// Work assigned to the recipient.
    Assignment,
    /// Human approval required.
    Approval,
    /// Approaching or missed due time.
    Deadline,
    /// Direct Team mention.
    Mention,
    /// Agent completed or failed.
    AgentOutcome,
    /// Important protected state transition.
    ImportantTransition,
}

/// Versioned notification preferences.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MkNotificationPreferencesV1 {
    /// Contract version. Must be `1`.
    pub version: u16,
    /// Enabled actionable classes. Duplicate values are invalid.
    pub enabled: Vec<MkNotificationClass>,
    /// Quiet-hours start as local minutes since midnight.
    pub quiet_start_minute: Option<u16>,
    /// Quiet-hours end as local minutes since midnight.
    pub quiet_end_minute: Option<u16>,
    /// IANA timezone label resolved by the delivery scheduler.
    pub timezone: Option<String>,
}

/// Inputs that identify one semantic notification without including content.
pub struct MkNotificationDedupeInput<'a> {
    /// Community UUID.
    pub community_id: Uuid,
    /// Recipient public key bytes.
    pub recipient_pubkey: &'a [u8],
    /// Notification class.
    pub class: MkNotificationClass,
    /// Target event kind.
    pub kind: u32,
    /// Stable target entity UUID.
    pub entity_id: Uuid,
    /// Target entity version or transition sequence.
    pub version: i64,
}

/// Notification preference validation error.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum MkNotificationError {
    /// Unsupported version.
    #[error("unsupported notification preference version")]
    UnsupportedVersion,
    /// Quiet-hours values are incomplete or invalid.
    #[error("invalid quiet-hours configuration")]
    InvalidQuietHours,
    /// Enabled class is duplicated.
    #[error("duplicate notification class")]
    DuplicateClass,
}

impl MkNotificationPreferencesV1 {
    /// Validate preference shape.
    pub fn validate(&self) -> Result<(), MkNotificationError> {
        if self.version != 1 {
            return Err(MkNotificationError::UnsupportedVersion);
        }
        let unique: HashSet<_> = self.enabled.iter().copied().collect();
        if unique.len() != self.enabled.len() {
            return Err(MkNotificationError::DuplicateClass);
        }
        match (
            self.quiet_start_minute,
            self.quiet_end_minute,
            self.timezone.as_deref(),
        ) {
            (None, None, None) => {}
            (Some(start), Some(end), Some(timezone))
                if start < 1440
                    && end < 1440
                    && start != end
                    && !timezone.trim().is_empty()
                    && timezone.len() <= 80 => {}
            _ => return Err(MkNotificationError::InvalidQuietHours),
        }
        Ok(())
    }

    /// Whether a class is enabled and not currently suppressed by quiet hours.
    ///
    /// `local_minute` is derived by the scheduler from the validated IANA
    /// timezone. Callers pass `None` when quiet hours are not configured.
    pub fn should_deliver(&self, class: MkNotificationClass, local_minute: Option<u16>) -> bool {
        if !self.enabled.contains(&class) {
            return false;
        }
        let (Some(start), Some(end)) = (self.quiet_start_minute, self.quiet_end_minute) else {
            return true;
        };
        let Some(minute) = local_minute.filter(|minute| *minute < 1440) else {
            // A scheduler that cannot resolve local time must not bypass a
            // user's configured quiet hours.
            return false;
        };
        let quiet = if start < end {
            minute >= start && minute < end
        } else {
            minute >= start || minute < end
        };
        !quiet
    }
}

/// Create a stable, content-free semantic deduplication key.
pub fn mk_notification_dedupe_key(input: MkNotificationDedupeInput<'_>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input.community_id.as_bytes());
    hasher.update(input.recipient_pubkey);
    hasher.update([notification_class_discriminator(input.class)]);
    hasher.update(input.kind.to_be_bytes());
    hasher.update(input.entity_id.as_bytes());
    hasher.update(input.version.to_be_bytes());
    hasher.finalize().into()
}

const fn notification_class_discriminator(class: MkNotificationClass) -> u8 {
    match class {
        MkNotificationClass::Assignment => 1,
        MkNotificationClass::Approval => 2,
        MkNotificationClass::Deadline => 3,
        MkNotificationClass::Mention => 4,
        MkNotificationClass::AgentOutcome => 5,
        MkNotificationClass::ImportantTransition => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preferences(start: u16, end: u16) -> MkNotificationPreferencesV1 {
        MkNotificationPreferencesV1 {
            version: 1,
            enabled: vec![MkNotificationClass::Approval],
            quiet_start_minute: Some(start),
            quiet_end_minute: Some(end),
            timezone: Some("America/Los_Angeles".into()),
        }
    }

    #[test]
    fn overnight_quiet_hours_cross_midnight() {
        let prefs = preferences(22 * 60, 7 * 60);
        assert!(prefs.validate().is_ok());
        assert!(!prefs.should_deliver(MkNotificationClass::Approval, Some(23 * 60)));
        assert!(!prefs.should_deliver(MkNotificationClass::Approval, Some(6 * 60)));
        assert!(prefs.should_deliver(MkNotificationClass::Approval, Some(12 * 60)));
    }

    #[test]
    fn dedupe_is_fenced_by_community_and_recipient() {
        let entity = Uuid::new_v4();
        let key = |community, recipient: &[u8]| {
            mk_notification_dedupe_key(MkNotificationDedupeInput {
                community_id: community,
                recipient_pubkey: recipient,
                class: MkNotificationClass::Assignment,
                kind: 30802,
                entity_id: entity,
                version: 3,
            })
        };
        let community_a = Uuid::new_v4();
        assert_ne!(key(community_a, &[1; 32]), key(Uuid::new_v4(), &[1; 32]));
        assert_ne!(key(community_a, &[1; 32]), key(community_a, &[2; 32]));
    }
}
