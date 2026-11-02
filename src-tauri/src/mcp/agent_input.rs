//! Reassembly of agent (and human) terminal input into the commands that actually
//! reach the shell.
//!
//! A single `session_send_input` call may carry part of a line, and Enter may arrive
//! in a later one, so the audit trail and the GUI notice both need the submitted line
//! reconstructed from the byte stream. Escape, tab-completion and cursor movement
//! rewrite the line in ways input bytes cannot reveal, so those sessions are marked
//! unverifiable rather than guessed at.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

const MAX_TRACKED_COMMAND_BYTES: usize = 64 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedAgentCommand {
    pub command: String,
    /// False when shell history, completion, cursor movement, or another
    /// terminal control may have changed the command outside this tracker.
    pub is_verifiable: bool,
}

#[derive(Debug, Clone)]
struct TrackedInput {
    buffer: String,
    is_verifiable: bool,
}

impl Default for TrackedInput {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            is_verifiable: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentInputTracker {
    buffers: HashMap<String, TrackedInput>,
    sensitive_sessions: HashSet<String>,
}

pub struct AgentInputCheckpoint {
    session_id: String,
    input: Option<TrackedInput>,
    sensitive: bool,
}

/// Coordinates mixed human/AI input without letting one session's input
/// transaction freeze unrelated terminals.
#[derive(Default)]
pub struct SharedAgentInputTracker {
    tracker: AsyncMutex<AgentInputTracker>,
    session_locks: StdMutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl SharedAgentInputTracker {
    pub async fn lock_session(&self, session_id: &str) -> OwnedMutexGuard<()> {
        let session_lock = self
            .session_locks
            .lock()
            .expect("session input lock map poisoned")
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone();
        session_lock.lock_owned().await
    }

    pub async fn checkpoint_and_observe(
        &self,
        session_id: &str,
        data: &str,
        keys: &[String],
        append_enter: bool,
    ) -> (AgentInputCheckpoint, Vec<TrackedAgentCommand>) {
        let (checkpoint, commands, _) = self
            .checkpoint_and_observe_sensitive(session_id, data, keys, append_enter, false)
            .await;
        (checkpoint, commands)
    }

    pub async fn checkpoint_and_observe_sensitive(
        &self,
        session_id: &str,
        data: &str,
        keys: &[String],
        append_enter: bool,
        sensitive: bool,
    ) -> (AgentInputCheckpoint, Vec<TrackedAgentCommand>, bool) {
        let mut tracker = self.tracker.lock().await;
        let checkpoint = tracker.checkpoint(session_id);
        if sensitive {
            tracker.sensitive_sessions.insert(session_id.to_string());
        }
        let redact = tracker.sensitive_sessions.contains(session_id);
        let commands = tracker.observe(session_id, data, keys, append_enter);
        if !tracker.buffers.contains_key(session_id) {
            tracker.sensitive_sessions.remove(session_id);
        }
        (checkpoint, commands, redact)
    }

    pub async fn restore(&self, checkpoint: AgentInputCheckpoint) {
        self.tracker.lock().await.restore(checkpoint);
    }
}

impl AgentInputTracker {
    fn checkpoint(&self, session_id: &str) -> AgentInputCheckpoint {
        AgentInputCheckpoint {
            session_id: session_id.to_string(),
            input: self.buffers.get(session_id).cloned(),
            sensitive: self.sensitive_sessions.contains(session_id),
        }
    }

    fn restore(&mut self, checkpoint: AgentInputCheckpoint) {
        if checkpoint.sensitive {
            self.sensitive_sessions
                .insert(checkpoint.session_id.clone());
        } else {
            self.sensitive_sessions.remove(&checkpoint.session_id);
        }
        if let Some(input) = checkpoint.input {
            self.buffers.insert(checkpoint.session_id, input);
        } else {
            self.buffers.remove(&checkpoint.session_id);
        }
    }

    pub fn observe(
        &mut self,
        session_id: &str,
        data: &str,
        keys: &[String],
        append_enter: bool,
    ) -> Vec<TrackedAgentCommand> {
        let mut executed = Vec::new();
        let input = self.buffers.entry(session_id.to_string()).or_default();

        for ch in data.chars() {
            match ch {
                '\r' | '\n' => finish_command(input, &mut executed),
                '\u{7f}' | '\u{8}' => {
                    input.buffer.pop();
                }
                '\u{3}' => reset_input(input),
                // Completion and terminal escape/control sequences can rewrite
                // the line in shell-specific ways that cannot be reconstructed
                // from input bytes alone, so record the line as unverifiable.
                '\t' => input.is_verifiable = false,
                ch if !ch.is_control() => input.buffer.push(ch),
                _ => input.is_verifiable = false,
            }
            input.is_verifiable &= input.buffer.len() <= MAX_TRACKED_COMMAND_BYTES;
            trim_tracked_buffer(&mut input.buffer);
        }

        for key in keys {
            match key.trim().to_ascii_lowercase().as_str() {
                "enter" => finish_command(input, &mut executed),
                "backspace" => {
                    input.buffer.pop();
                }
                "ctrl-c" => reset_input(input),
                "tab" | "escape" | "esc" | "up" | "down" | "right" | "left" => {
                    input.is_verifiable = false;
                }
                // These control keys affect the foreground process or shell,
                // but do not produce a line that the shell executes on Enter.
                "ctrl-d" | "ctrl-z" => {}
                _ => input.is_verifiable = false,
            }
            input.is_verifiable &= input.buffer.len() <= MAX_TRACKED_COMMAND_BYTES;
            trim_tracked_buffer(&mut input.buffer);
        }

        if append_enter {
            finish_command(input, &mut executed);
        }

        if input.buffer.is_empty() && input.is_verifiable {
            self.buffers.remove(session_id);
        }
        executed
    }
}

fn finish_command(input: &mut TrackedInput, executed: &mut Vec<TrackedAgentCommand>) {
    let command = input.buffer.trim();
    if !command.is_empty() || !input.is_verifiable {
        executed.push(TrackedAgentCommand {
            command: if command.is_empty() {
                "Interactive shell command (final text is not observable)".to_string()
            } else {
                command.to_string()
            },
            is_verifiable: input.is_verifiable,
        });
    }
    reset_input(input);
}

fn reset_input(input: &mut TrackedInput) {
    input.buffer.clear();
    input.is_verifiable = true;
}

fn trim_tracked_buffer(buffer: &mut String) {
    if buffer.len() <= MAX_TRACKED_COMMAND_BYTES {
        return;
    }

    let mut keep_from = buffer.len() - MAX_TRACKED_COMMAND_BYTES;
    while !buffer.is_char_boundary(keep_from) {
        keep_from += 1;
    }
    buffer.drain(..keep_from);
}

/// Extract the command text that will actually execute from a
/// `session_send_input` payload.
///
/// Returns `None` when the input will not execute yet (e.g. the agent is only
/// typing without pressing Enter), so partial keystrokes are never gated.
pub fn extract_executed_command(data: &str, keys: &[String], append_enter: bool) -> Option<String> {
    let has_enter_key = keys
        .iter()
        .any(|key| matches!(key.trim().to_ascii_lowercase().as_str(), "enter"));
    let data_has_newline = data.contains('\r') || data.contains('\n');

    if !(append_enter || has_enter_key || data_has_newline) {
        return None;
    }

    let command = data.trim_end_matches(['\r', '\n']).trim();
    if command.is_empty() {
        return None;
    }

    Some(command.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_only_returns_executed_commands() {
        assert_eq!(
            extract_executed_command("rm -rf /tmp/x", &[], true),
            Some("rm -rf /tmp/x".to_string())
        );
        assert_eq!(
            extract_executed_command("ls\n", &[], false),
            Some("ls".to_string())
        );
        assert_eq!(
            extract_executed_command("whoami", &["enter".to_string()], false),
            Some("whoami".to_string())
        );
        // Pure typing (no Enter) is never gated.
        assert_eq!(extract_executed_command("rm -rf /tmp/x", &[], false), None);
        // Enter with no buffered text yields nothing to classify.
        assert_eq!(
            extract_executed_command("", &["enter".to_string()], false),
            None
        );
    }
    #[test]
    fn tracks_commands_across_multiple_input_calls() {
        let mut tracker = AgentInputTracker::default();
        assert!(tracker
            .observe("s1", "rm -rf /tmp/x", &[], false)
            .is_empty());
        assert_eq!(
            tracker.observe("s1", "", &["enter".to_string()], false),
            vec![TrackedAgentCommand {
                command: "rm -rf /tmp/x".to_string(),
                is_verifiable: true,
            }]
        );
    }

    #[test]
    fn tracks_editing_and_multiple_commands() {
        let mut tracker = AgentInputTracker::default();
        assert_eq!(
            tracker.observe("s1", "echp\u{7f}o one\nwhoami\r", &[], false),
            vec![
                TrackedAgentCommand {
                    command: "echo one".to_string(),
                    is_verifiable: true,
                },
                TrackedAgentCommand {
                    command: "whoami".to_string(),
                    is_verifiable: true,
                },
            ]
        );
        assert!(tracker
            .observe("s1", "discard me\u{3}", &[], false)
            .is_empty());
        assert!(tracker
            .observe("s1", "", &["enter".to_string()], false)
            .is_empty());
    }

    #[test]
    fn history_and_cursor_editing_make_the_submitted_command_unverifiable() {
        let mut tracker = AgentInputTracker::default();

        assert!(tracker
            .observe("history", "", &["up".to_string()], false)
            .is_empty());
        assert_eq!(
            tracker.observe("history", "", &["enter".to_string()], false),
            vec![TrackedAgentCommand {
                command: "Interactive shell command (final text is not observable)".to_string(),
                is_verifiable: false,
            }]
        );

        assert!(tracker
            .observe("edit", "echo safe", &["left".to_string()], false)
            .is_empty());
        let edited = tracker.observe("edit", "rm -rf /", &["enter".to_string()], false);
        assert_eq!(edited.len(), 1);
        assert!(!edited[0].is_verifiable);
    }

    #[test]
    fn raw_terminal_controls_and_completion_taint_the_command() {
        let mut tracker = AgentInputTracker::default();
        let raw_history = tracker.observe("s1", "\u{1b}[A\r", &[], false);
        assert_eq!(raw_history.len(), 1);
        assert!(!raw_history[0].is_verifiable);

        let completed = tracker.observe("s2", "rm\t\r", &[], false);
        assert_eq!(completed.len(), 1);
        assert!(!completed[0].is_verifiable);
    }

    #[test]
    fn tracker_checkpoint_restores_a_split_command_after_failure() {
        let mut tracker = AgentInputTracker::default();
        assert!(tracker
            .observe("s1", "rm -rf /tmp/x", &[], false)
            .is_empty());

        let checkpoint = tracker.clone();
        let first_attempt = tracker.observe("s1", "", &["enter".to_string()], false);
        assert_eq!(first_attempt[0].command, "rm -rf /tmp/x");

        tracker = checkpoint;
        let retry = tracker.observe("s1", "", &["enter".to_string()], false);
        assert_eq!(retry, first_attempt);
    }

    #[test]
    fn shared_tracker_rolls_back_only_the_failed_session() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let shared = SharedAgentInputTracker::default();
            let _ = shared
                .checkpoint_and_observe("s1", "rm -rf /tmp/a", &[], false)
                .await;
            let (failed_s2, _) = shared
                .checkpoint_and_observe("s2", "echo never-sent", &[], false)
                .await;
            shared.restore(failed_s2).await;

            let (_, s1) = shared
                .checkpoint_and_observe("s1", "", &["enter".to_string()], false)
                .await;
            let (_, s2) = shared
                .checkpoint_and_observe("s2", "", &["enter".to_string()], false)
                .await;
            assert_eq!(s1[0].command, "rm -rf /tmp/a");
            assert!(s2.is_empty());
        });
    }

    #[tokio::test]
    async fn secret_redaction_survives_split_enter_and_failed_input_rollback() {
        let tracker = SharedAgentInputTracker::default();
        let (_, commands, redact) = tracker
            .checkpoint_and_observe_sensitive("secret", "temporary-secret", &[], false, true)
            .await;
        assert!(commands.is_empty() && redact);
        let (checkpoint, commands, redact) = tracker
            .checkpoint_and_observe_sensitive("secret", "\r", &[], false, false)
            .await;
        assert!(redact);
        assert_eq!(commands.len(), 1);
        tracker.restore(checkpoint).await;
        let (_, commands, redact) = tracker
            .checkpoint_and_observe_sensitive("secret", "", &["enter".into()], false, false)
            .await;
        assert!(redact && commands.len() == 1);
        let (_, commands, redact) = tracker
            .checkpoint_and_observe_sensitive("secret", "printf next\r", &[], false, false)
            .await;
        assert!(!redact);
        assert_eq!(commands[0].command, "printf next");
    }
    #[test]
    fn oversized_input_is_never_misrepresented_as_fully_verified() {
        let mut tracker = AgentInputTracker::default();
        let commands = tracker.observe(
            "oversize",
            &("界".repeat(MAX_TRACKED_COMMAND_BYTES) + "\n"),
            &[],
            false,
        );
        assert_eq!(commands.len(), 1);
        assert!(!commands[0].is_verifiable);
        assert!(commands[0].command.len() <= MAX_TRACKED_COMMAND_BYTES);
    }

    #[test]
    fn input_lock_is_scoped_to_one_session() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let shared = SharedAgentInputTracker::default();
            let _s1 = shared.lock_session("s1").await;
            assert!(tokio::time::timeout(
                std::time::Duration::from_millis(50),
                shared.lock_session("s2")
            )
            .await
            .is_ok());
        });
    }
}
