use directories::ProjectDirs;
use ipc::{AuditEntry, ChatRequest, McpServerRecord, PendingConsentRecord, Session};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProviderState {
    pub active_provider: Option<String>,
    pub configs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingConsentState {
    pub record: PendingConsentRecord,
    pub chat_request: ChatRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProjectState {
    pub open_path: Option<String>,
}

pub trait Storage {
    fn list_sessions(&self) -> io::Result<Vec<Session>>;
    fn write_sessions(&self, sessions: &[Session]) -> io::Result<()>;

    fn read_provider_state(&self) -> io::Result<ProviderState>;
    fn write_provider_state(&self, state: &ProviderState) -> io::Result<()>;

    fn read_audit_entries(&self) -> io::Result<Vec<AuditEntry>>;
    fn write_audit_entries(&self, entries: &[AuditEntry]) -> io::Result<()>;

    fn read_pending_consents(&self) -> io::Result<Vec<PendingConsentState>>;
    fn write_pending_consents(&self, entries: &[PendingConsentState]) -> io::Result<()>;

    fn read_mcp_servers(&self) -> io::Result<Vec<McpServerRecord>>;
    fn write_mcp_servers(&self, entries: &[McpServerRecord]) -> io::Result<()>;

    fn read_project_state(&self) -> io::Result<ProjectState>;
    fn write_project_state(&self, state: &ProjectState) -> io::Result<()>;
}

#[derive(Clone, Debug)]
pub struct FileStorage {
    root: PathBuf,
}

impl FileStorage {
    pub fn new_default() -> io::Result<Self> {
        let proj = ProjectDirs::from("com", "cmnd-n-ctrl", "cmnd-n-ctrl")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unable to resolve app data dir"))?;
        Self::new_in_dir(proj.data_local_dir())
    }

    pub fn new_in_dir(path: impl AsRef<Path>) -> io::Result<Self> {
        let root = path.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path_for(&self, file_name: &str) -> PathBuf {
        self.root.join(file_name)
    }

    fn read_json<T>(&self, file_name: &str) -> io::Result<T>
    where
        T: DeserializeOwned + Default,
    {
        let path = self.path_for(file_name);
        if !path.exists() {
            return Ok(T::default());
        }
        let raw = fs::read_to_string(path)?;
        serde_json::from_str(&raw).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse {}: {err}", file_name),
            )
        })
    }

    fn write_json<T>(&self, file_name: &str, value: &T) -> io::Result<()>
    where
        T: Serialize,
    {
        let path = self.path_for(file_name);
        let tmp = path.with_extension("tmp");
        let payload = serde_json::to_string_pretty(value)
            .map_err(|err| io::Error::other(format!("serialize {file_name}: {err}")))?;
        fs::write(&tmp, payload)?;
        fs::rename(tmp, path)?;
        Ok(())
    }
}

impl Storage for FileStorage {
    fn list_sessions(&self) -> io::Result<Vec<Session>> {
        self.read_json("sessions.json")
    }

    fn write_sessions(&self, sessions: &[Session]) -> io::Result<()> {
        self.write_json("sessions.json", &sessions)
    }

    fn read_provider_state(&self) -> io::Result<ProviderState> {
        self.read_json("providers.json")
    }

    fn write_provider_state(&self, state: &ProviderState) -> io::Result<()> {
        self.write_json("providers.json", state)
    }

    fn read_audit_entries(&self) -> io::Result<Vec<AuditEntry>> {
        self.read_json("audit.json")
    }

    fn write_audit_entries(&self, entries: &[AuditEntry]) -> io::Result<()> {
        self.write_json("audit.json", &entries)
    }

    fn read_pending_consents(&self) -> io::Result<Vec<PendingConsentState>> {
        self.read_json("pending_consents.json")
    }

    fn write_pending_consents(&self, entries: &[PendingConsentState]) -> io::Result<()> {
        self.write_json("pending_consents.json", &entries)
    }

    fn read_mcp_servers(&self) -> io::Result<Vec<McpServerRecord>> {
        self.read_json("mcp_servers.json")
    }

    fn write_mcp_servers(&self, entries: &[McpServerRecord]) -> io::Result<()> {
        self.write_json("mcp_servers.json", &entries)
    }

    fn read_project_state(&self) -> io::Result<ProjectState> {
        self.read_json("project.json")
    }

    fn write_project_state(&self, state: &ProjectState) -> io::Result<()> {
        self.write_json("project.json", state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc::{ChatMessage, ChatMode, ProviderConfig};
    use tempfile::tempdir;

    #[test]
    fn session_storage_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let store = FileStorage::new_in_dir(dir.path()).expect("store");
        let session = Session {
            id: "sess-1".to_string(),
            created_at_unix_seconds: 1,
            updated_at_unix_seconds: 1,
            title: "Test".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
        };
        store.write_sessions(&[session.clone()]).expect("write");
        let got = store.list_sessions().expect("read");
        assert_eq!(got, vec![session]);
    }

    #[test]
    fn pending_consent_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let store = FileStorage::new_in_dir(dir.path()).expect("store");
        let item = PendingConsentState {
            record: PendingConsentRecord {
                consent_id: "consent-1".to_string(),
                session_id: None,
                requested_at_unix_seconds: 1,
                tool_name: "desktop.app.activate".to_string(),
                capability_tier: "SystemActions".to_string(),
                status: "pending".to_string(),
                rationale: "requires explicit consent".to_string(),
                arguments_preview: Some("{\"app\":\"x\"}".to_string()),
                request_fingerprint: "req-1".to_string(),
            },
            chat_request: ChatRequest {
                messages: vec![],
                provider_config: ProviderConfig {
                    provider_name: "openai-stub".to_string(),
                    model: None,
                },
                mode: ChatMode::RequireConfirmation,
            },
        };
        store.write_pending_consents(&[item.clone()]).expect("write");
        let got = store.read_pending_consents().expect("read");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].record.consent_id, item.record.consent_id);
    }
}
