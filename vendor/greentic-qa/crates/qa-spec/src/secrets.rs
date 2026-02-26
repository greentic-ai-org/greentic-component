use globset::Glob;

use crate::spec::form::SecretsPolicy;

/// Secret access modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretAction {
    Read,
    Write,
}

/// Result of evaluating a secret policy for a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretAccessResult {
    Allowed,
    Denied(&'static str),
    HostUnavailable,
}

fn matches_pattern(pattern: &str, key: &str) -> bool {
    match Glob::new(pattern) {
        Ok(glob) => glob.compile_matcher().is_match(key),
        Err(_) => false,
    }
}

fn matches_any(patterns: &[String], key: &str) -> bool {
    patterns.iter().any(|pattern| matches_pattern(pattern, key))
}

pub fn evaluate(
    policy: Option<&SecretsPolicy>,
    key: &str,
    action: SecretAction,
    host_available: bool,
) -> SecretAccessResult {
    let policy = match policy {
        Some(policy) if policy.enabled => policy,
        _ => return SecretAccessResult::Denied("secret_access_denied"),
    };

    let enabled = match action {
        SecretAction::Read => policy.read_enabled,
        SecretAction::Write => policy.write_enabled,
    };

    if !enabled {
        return SecretAccessResult::Denied("secret_access_denied");
    }

    if matches_any(&policy.deny, key) {
        return SecretAccessResult::Denied("secret_access_denied");
    }

    if policy.allow.is_empty() || !matches_any(&policy.allow, key) {
        return SecretAccessResult::Denied("secret_access_denied");
    }

    if !host_available {
        return SecretAccessResult::HostUnavailable;
    }

    SecretAccessResult::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::form::SecretsPolicy;

    fn policy() -> SecretsPolicy {
        SecretsPolicy {
            enabled: true,
            read_enabled: true,
            write_enabled: true,
            allow: vec!["aws/*".into()],
            deny: vec!["aws/secret-deny".into()],
        }
    }

    #[test]
    fn allowed_key_using_pattern() {
        assert_eq!(
            evaluate(Some(&policy()), "aws/key", SecretAction::Read, true),
            SecretAccessResult::Allowed
        );
    }

    #[test]
    fn denied_key_due_to_deny_list() {
        assert_eq!(
            evaluate(Some(&policy()), "aws/secret-deny", SecretAction::Read, true),
            SecretAccessResult::Denied("secret_access_denied")
        );
    }

    #[test]
    fn host_unavailable_when_disabled() {
        assert_eq!(
            evaluate(Some(&policy()), "aws/key", SecretAction::Read, false),
            SecretAccessResult::HostUnavailable
        );
    }
}
