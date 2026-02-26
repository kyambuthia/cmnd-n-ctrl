pub mod orchestrator;
pub mod policy;
pub mod tool_registry;

use actions::traits::StubActionBackend;
use ipc::{
    ActionEvent, AuditEntry, AuditGetRequest, AuditListRequest, ChatApproveRequest, ChatDenyRequest,
    ChatRequest, ChatResponse, ChatService, ConsentActionRequest, ConsentListRequest, ConsentRequest,
    McpServerAddRequest, McpServerMutationResponse, McpServerRecord, McpServerRemoveRequest,
    McpServerStateRequest, PendingConsentRecord, ProjectOpenRequest, ProjectOpenResponse,
    ProjectStatusRequest, ProjectStatusResponse, ProviderConfigGetRequest, ProviderConfigRecord,
    ProviderConfigSetRequest, ProviderConfigSetResponse, ProviderInfo, ProvidersSetRequest, Session,
    SessionCreateRequest, SessionDeleteRequest, SessionDeleteResponse, SessionGetRequest,
    SessionMessagesAppendRequest, SessionMessagesAppendResponse, SessionSummary, SystemHealthResponse,
    Tool,
};
use providers::ProviderChoice;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use storage::{FileStorage, PendingConsentState, ProjectState, ProviderState, Storage};

use crate::orchestrator::Orchestrator;
use crate::policy::Policy;
use crate::tool_registry::ToolRegistry;

pub struct AgentService {
    orchestrator: Orchestrator<ProviderChoice, StubActionBackend>,
    tool_registry: ToolRegistry,
    storage: FileStorage,
    platform: &'static str,
    synthetic_audit_counter: u64,
    consent_counter: u64,
    session_counter: u64,
    mcp_counter: u64,
    mcp_processes: RefCell<HashMap<String, Child>>,
}

impl AgentService {
    const CONSENT_TTL_SECS: u64 = 300;

    pub fn new_for_platform(platform: &'static str) -> Self {
        Self::new_for_platform_with_storage(platform, None)
    }

    pub fn new_for_platform_with_storage_dir(
        platform: &'static str,
        dir: impl AsRef<Path>,
    ) -> Self {
        Self::new_for_platform_with_storage(platform, Some(dir.as_ref()))
    }

    fn new_for_platform_with_storage(platform: &'static str, storage_dir: Option<&Path>) -> Self {
        let tool_registry = ToolRegistry::new_default();
        let orchestrator = Orchestrator::new(
            Policy::default(),
            tool_registry.clone(),
            ProviderChoice::by_name("openai-stub"),
            StubActionBackend::new(platform),
        );
        let storage = if let Some(dir) = storage_dir {
            FileStorage::new_in_dir(dir).expect("custom file storage")
        } else {
            FileStorage::new_default().unwrap_or_else(|_| {
                let fallback = env::temp_dir().join("cmnd-n-ctrl-local-data");
                FileStorage::new_in_dir(fallback).expect("fallback file storage")
            })
        };
        let mut svc = Self {
            orchestrator,
            tool_registry,
            storage,
            platform,
            synthetic_audit_counter: 0,
            consent_counter: 0,
            session_counter: 0,
            mcp_counter: 0,
            mcp_processes: RefCell::new(HashMap::new()),
        };
        svc.hydrate_counters();
        let _ = svc.normalize_mcp_statuses_on_startup();
        svc
    }

    fn hydrate_counters(&mut self) {
        if let Ok(items) = self.storage.read_pending_consents() {
            self.consent_counter = items
                .iter()
                .filter_map(|c| c.record.consent_id.strip_prefix("consent-"))
                .filter_map(|s| s.parse::<u64>().ok())
                .max()
                .unwrap_or(0);
        }
        if let Ok(items) = self.storage.list_sessions() {
            self.session_counter = items
                .iter()
                .filter_map(|s| s.id.strip_prefix("sess-"))
                .filter_map(|s| s.parse::<u64>().ok())
                .max()
                .unwrap_or(0);
        }
        if let Ok(items) = self.storage.read_mcp_servers() {
            self.mcp_counter = items
                .iter()
                .filter_map(|s| s.id.strip_prefix("mcp-"))
                .filter_map(|s| s.parse::<u64>().ok())
                .max()
                .unwrap_or(0);
        }
        if let Ok(items) = self.storage.read_audit_entries() {
            self.synthetic_audit_counter = items
                .iter()
                .filter_map(|a| a.audit_id.rsplit('-').next())
                .filter_map(|s| s.parse::<u64>().ok())
                .max()
                .unwrap_or(0);
        }
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn next_synthetic_audit_id(&mut self) -> String {
        self.synthetic_audit_counter += 1;
        format!("audit-{:06}", self.synthetic_audit_counter)
    }

    fn next_consent_id(&mut self) -> String {
        self.consent_counter += 1;
        format!("consent-{:06}", self.consent_counter)
    }

    fn next_session_id(&mut self) -> String {
        self.session_counter += 1;
        format!("sess-{:06}", self.session_counter)
    }

    fn next_mcp_id(&mut self) -> String {
        self.mcp_counter += 1;
        format!("mcp-{:06}", self.mcp_counter)
    }

    fn rebuild_orchestrator(&mut self, provider_name: &str) {
        let provider = ProviderChoice::by_name(provider_name);
        let project_root = self
            .storage
            .read_project_state()
            .ok()
            .and_then(|s| s.open_path)
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        self.orchestrator = Orchestrator::new(
            Policy::default(),
            self.tool_registry.clone(),
            provider,
            StubActionBackend::with_project_root(self.platform, project_root),
        );
    }

    fn io_err(err: std::io::Error) -> String {
        err.to_string()
    }

    fn read_sessions(&self) -> Result<Vec<Session>, String> {
        self.storage.list_sessions().map_err(Self::io_err)
    }

    fn write_sessions(&self, sessions: &[Session]) -> Result<(), String> {
        self.storage.write_sessions(sessions).map_err(Self::io_err)
    }

    fn read_pending_consents(&self) -> Result<Vec<PendingConsentState>, String> {
        self.storage.read_pending_consents().map_err(Self::io_err)
    }

    fn write_pending_consents(&self, items: &[PendingConsentState]) -> Result<(), String> {
        self.storage.write_pending_consents(items).map_err(Self::io_err)
    }

    fn persist_audit_from_response(&mut self, response: &ChatResponse, provider: &str) {
        let mut audits = match self.storage.read_audit_entries() {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };
        let policy_decisions = response
            .proposed_actions
            .iter()
            .map(|evt| {
                format!(
                    "{}:{}:{}",
                    evt.tool_name,
                    evt.status,
                    evt.reason.clone().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        let proposed_tool_calls = response
            .proposed_actions
            .iter()
            .map(|evt| evt.tool_name.clone())
            .collect::<Vec<_>>();
        let evidence_summaries = response
            .executed_action_events
            .iter()
            .filter_map(|evt| evt.evidence_summary.clone())
            .collect::<Vec<_>>();
        audits.push(AuditEntry {
            audit_id: response.audit_id.clone(),
            timestamp_unix_seconds: Self::now_secs(),
            session_id: response.session_id.clone(),
            provider: provider.to_string(),
            policy_decisions,
            proposed_tool_calls,
            executed_actions: response.actions_executed.clone(),
            evidence_summaries,
        });
        let _ = self.storage.write_audit_entries(&audits);
    }

    fn attach_or_create_consent(
        &mut self,
        request: &ChatRequest,
        response: &mut ChatResponse,
    ) -> Result<(), String> {
        let pending_events = response
            .proposed_actions
            .iter()
            .filter(|evt| evt.status == "consent_required")
            .cloned()
            .collect::<Vec<_>>();
        if pending_events.is_empty() {
            response.consent_token = None;
            response.consent_request = None;
            return Ok(());
        }
        let mut items = self.read_pending_consents()?;
        let timestamp = Self::now_secs();
        let expires_at = timestamp.saturating_add(Self::CONSENT_TTL_SECS);
        let first = &pending_events[0];
        let consent_id = self.next_consent_id();
        items.push(PendingConsentState {
            record: PendingConsentRecord {
                consent_id: consent_id.clone(),
                session_id: request.session_id.clone(),
                requested_at_unix_seconds: timestamp,
                expires_at_unix_seconds: expires_at,
                tool_name: first.tool_name.clone(),
                capability_tier: first.capability_tier.clone(),
                status: "pending".to_string(),
                rationale: first
                    .reason
                    .clone()
                    .unwrap_or_else(|| "explicit consent required".to_string()),
                arguments_preview: first.arguments_preview.clone(),
                request_fingerprint: response.request_fingerprint.clone(),
            },
            chat_request: request.clone(),
        });
        self.write_pending_consents(&items)?;
        response.consent_token = Some(consent_id);
        response.consent_request = Some(build_consent_request(
            &response.proposed_actions,
            Some(expires_at),
            Some(Self::CONSENT_TTL_SECS),
        ));
        Ok(())
    }

    fn mark_or_find_pending_consent(
        &mut self,
        consent_id: &str,
        new_status: &str,
    ) -> Result<PendingConsentState, String> {
        let mut items = self.read_pending_consents()?;
        let idx = items
            .iter()
            .position(|item| item.record.consent_id == consent_id)
            .ok_or_else(|| "consent_not_found".to_string())?;
        let now = Self::now_secs();
        if items[idx].record.status != "pending" {
            return Err(format!(
                "consent_not_pending:{}",
                items[idx].record.status
            ));
        }
        if items[idx].record.expires_at_unix_seconds > 0 && now > items[idx].record.expires_at_unix_seconds {
            items[idx].record.status = "expired".to_string();
            let _ = self.write_pending_consents(&items);
            return Err("consent_expired".to_string());
        }
        items[idx].record.status = new_status.to_string();
        let out = items[idx].clone();
        self.write_pending_consents(&items)?;
        Ok(out)
    }

    fn response_for_denial(&mut self, pending: &PendingConsentState, provider_name: &str) -> ChatResponse {
        let audit_id = self.next_synthetic_audit_id();
        let event = ActionEvent {
            tool_name: pending.record.tool_name.clone(),
            capability_tier: pending.record.capability_tier.clone(),
            status: "denied".to_string(),
            reason: Some(pending.record.rationale.clone()),
            arguments_preview: pending.record.arguments_preview.clone(),
            evidence_summary: None,
        };
        let response = ChatResponse {
            final_text: "User denied consent for requested actions.".to_string(),
            audit_id,
            request_fingerprint: pending.record.request_fingerprint.clone(),
            execution_state: "denied".to_string(),
            consent_token: None,
            session_id: pending.record.session_id.clone(),
            consent_request: None,
            actions_executed: vec![format!(
                "denied:{}:{}",
                pending.record.tool_name, pending.record.rationale
            )],
            proposed_actions: vec![event.clone()],
            executed_action_events: vec![],
            action_events: vec![event],
        };
        self.persist_audit_from_response(&response, provider_name);
        response
    }

    fn append_messages_to_session_if_requested(&mut self, request: &ChatRequest) {
        let Some(session_id) = &request.session_id else {
            return;
        };
        let mut sessions = match self.storage.list_sessions() {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(s) = sessions.iter_mut().find(|s| &s.id == session_id) {
            s.messages.extend(request.messages.clone());
            s.updated_at_unix_seconds = Self::now_secs();
            let _ = self.storage.write_sessions(&sessions);
        }
    }

    fn append_assistant_message_to_session_if_requested(
        &mut self,
        session_id: Option<&str>,
        content: &str,
    ) {
        let Some(session_id) = session_id else {
            return;
        };
        let mut sessions = match self.storage.list_sessions() {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(s) = sessions.iter_mut().find(|s| s.id == session_id) {
            s.messages.push(ipc::ChatMessage {
                role: "assistant".to_string(),
                content: content.to_string(),
            });
            s.updated_at_unix_seconds = Self::now_secs();
            let _ = self.storage.write_sessions(&sessions);
        }
    }

    fn provider_state(&self) -> Result<ProviderState, String> {
        self.storage.read_provider_state().map_err(Self::io_err)
    }

    fn write_provider_state(&self, state: &ProviderState) -> Result<(), String> {
        self.storage.write_provider_state(state).map_err(Self::io_err)
    }
}

impl ChatService for AgentService {
    fn chat_request(&mut self, mut params: ChatRequest) -> ChatResponse {
        if params.provider_config.provider_name.trim().is_empty() {
            if let Ok(state) = self.provider_state() {
                if let Some(active) = state.active_provider {
                    params.provider_config.provider_name = active;
                }
            }
        }
        self.append_messages_to_session_if_requested(&params);
        self.rebuild_orchestrator(&params.provider_config.provider_name);
        let mut response = self.orchestrator.run(
            params.messages.clone(),
            params.provider_config.clone(),
            params.mode.clone(),
        );
        response.audit_id = self.next_synthetic_audit_id();
        response.session_id = params.session_id.clone();
        let _ = self.attach_or_create_consent(&params, &mut response);
        response.execution_state = if response.consent_token.is_some() {
            "awaiting_consent".to_string()
        } else {
            "completed".to_string()
        };
        self.append_assistant_message_to_session_if_requested(
            response.session_id.as_deref(),
            &response.final_text,
        );
        self.persist_audit_from_response(&response, &params.provider_config.provider_name);
        response
    }

    fn chat_approve(&mut self, params: ChatApproveRequest) -> Result<ChatResponse, String> {
        let pending = self.mark_or_find_pending_consent(&params.consent_token, "approved")?;
        let req = pending.chat_request.clone();
        self.rebuild_orchestrator(&req.provider_config.provider_name);
        let mut response =
            self.orchestrator
                .run_with_confirmation(req.messages, req.provider_config.clone(), req.mode, true);
        response.audit_id = self.next_synthetic_audit_id();
        response.session_id = pending.record.session_id.clone();
        response.execution_state = "completed".to_string();
        response.consent_token = None;
        response.consent_request = None;
        self.append_assistant_message_to_session_if_requested(
            response.session_id.as_deref(),
            &response.final_text,
        );
        self.persist_audit_from_response(&response, &req.provider_config.provider_name);
        Ok(response)
    }

    fn chat_deny(&mut self, params: ChatDenyRequest) -> Result<ChatResponse, String> {
        let pending = self.mark_or_find_pending_consent(&params.consent_token, "denied")?;
        let response = self.response_for_denial(
            &pending,
            &pending.chat_request.provider_config.provider_name,
        );
        self.append_assistant_message_to_session_if_requested(
            response.session_id.as_deref(),
            &response.final_text,
        );
        Ok(response)
    }

    fn sessions_create(&mut self, params: SessionCreateRequest) -> Result<Session, String> {
        let mut sessions = self.read_sessions()?;
        let now = Self::now_secs();
        let title = params
            .title
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("Session {}", self.session_counter + 1));
        let session = Session {
            id: self.next_session_id(),
            created_at_unix_seconds: now,
            updated_at_unix_seconds: now,
            title,
            messages: vec![],
        };
        sessions.push(session.clone());
        self.write_sessions(&sessions)?;
        Ok(session)
    }

    fn sessions_list(&self) -> Result<Vec<SessionSummary>, String> {
        let mut sessions = self.read_sessions()?;
        sessions.sort_by_key(|s| s.updated_at_unix_seconds);
        sessions.reverse();
        Ok(sessions
            .into_iter()
            .map(|s| SessionSummary {
                id: s.id,
                title: s.title,
                created_at_unix_seconds: s.created_at_unix_seconds,
                updated_at_unix_seconds: s.updated_at_unix_seconds,
                message_count: s.messages.len(),
            })
            .collect())
    }

    fn sessions_get(&self, params: SessionGetRequest) -> Result<Session, String> {
        self.read_sessions()?
            .into_iter()
            .find(|s| s.id == params.session_id)
            .ok_or_else(|| "session not found".to_string())
    }

    fn sessions_delete(&mut self, params: SessionDeleteRequest) -> Result<SessionDeleteResponse, String> {
        let mut sessions = self.read_sessions()?;
        let before = sessions.len();
        sessions.retain(|s| s.id != params.session_id);
        self.write_sessions(&sessions)?;
        Ok(SessionDeleteResponse {
            deleted: sessions.len() != before,
        })
    }

    fn sessions_messages_append(
        &mut self,
        params: SessionMessagesAppendRequest,
    ) -> Result<SessionMessagesAppendResponse, String> {
        let mut sessions = self.read_sessions()?;
        let session = sessions
            .iter_mut()
            .find(|s| s.id == params.session_id)
            .ok_or_else(|| "session not found".to_string())?;
        session.messages.extend(params.messages);
        session.updated_at_unix_seconds = Self::now_secs();
        let out = session.clone();
        self.write_sessions(&sessions)?;
        Ok(SessionMessagesAppendResponse { session: out })
    }

    fn providers_list(&self) -> Result<Vec<ProviderInfo>, String> {
        let state = self.provider_state().unwrap_or_default();
        let mut names = BTreeSet::<String>::new();
        for name in ProviderChoice::builtin_names() {
            names.insert((*name).to_string());
        }
        for name in state.configs.keys() {
            names.insert(name.clone());
        }
        if let Some(active) = state.active_provider.as_ref() {
            names.insert(active.clone());
        }

        Ok(names
            .iter()
            .map(|name| {
                let cfg = state.configs.get(name).cloned().unwrap_or_else(|| "{}".to_string());
                let has_auth = provider_config_has_auth(name, &cfg);
                let auth_source = provider_config_auth_source(&cfg);
                ProviderInfo {
                    name: name.clone(),
                    enabled: true,
                    is_active: state.active_provider.as_deref() == Some(name.as_str()),
                    has_auth,
                    config_summary: if has_auth {
                        match auth_source.as_deref() {
                            Some("env") => "configured (env)".to_string(),
                            _ => "configured".to_string(),
                        }
                    } else {
                        "not configured".to_string()
                    },
                }
            })
            .collect())
    }

    fn providers_set(&mut self, params: ProvidersSetRequest) -> Result<ProviderInfo, String> {
        let mut state = self.provider_state().unwrap_or_default();
        state.active_provider = Some(params.provider_name.clone());
        self.write_provider_state(&state)?;
        self.providers_list()?
            .into_iter()
            .find(|p| p.name == params.provider_name)
            .ok_or_else(|| "provider not found".to_string())
    }

    fn providers_config_get(&self, params: ProviderConfigGetRequest) -> Result<ProviderConfigRecord, String> {
        let state = self.provider_state().unwrap_or_default();
        let provider_name = params
            .provider_name
            .or_else(|| state.active_provider.clone())
            .unwrap_or_else(|| "openai-stub".to_string());
        Ok(ProviderConfigRecord {
            provider_name: provider_name.clone(),
            is_active: state.active_provider.as_deref() == Some(provider_name.as_str()),
            config_json: redact_provider_config_json(
                &state
                    .configs
                    .get(&provider_name)
                    .cloned()
                    .unwrap_or_else(|| "{}".to_string()),
            ),
        })
    }

    fn providers_config_set(
        &mut self,
        params: ProviderConfigSetRequest,
    ) -> Result<ProviderConfigSetResponse, String> {
        let mut state = self.provider_state().unwrap_or_default();
        state
            .configs
            .insert(params.provider_name.clone(), params.config_json.clone());
        if state.active_provider.is_none() {
            state.active_provider = Some(params.provider_name.clone());
        }
        self.write_provider_state(&state)?;
        Ok(ProviderConfigSetResponse {
            provider_name: params.provider_name.clone(),
            has_auth: provider_config_has_auth(&params.provider_name, &params.config_json),
        })
    }

    fn mcp_servers_list(&self) -> Result<Vec<McpServerRecord>, String> {
        let _ = self.refresh_mcp_runtime_statuses();
        self.storage.read_mcp_servers().map_err(Self::io_err)
    }

    fn mcp_servers_add(&mut self, params: McpServerAddRequest) -> Result<McpServerMutationResponse, String> {
        let mut items = self.storage.read_mcp_servers().map_err(Self::io_err)?;
        let record = McpServerRecord {
            id: self.next_mcp_id(),
            name: params.name,
            command: params.command,
            args: params.args,
            status: "stopped".to_string(),
        };
        items.push(record.clone());
        self.storage.write_mcp_servers(&items).map_err(Self::io_err)?;
        Ok(McpServerMutationResponse {
            ok: true,
            server: Some(record),
        })
    }

    fn mcp_servers_remove(
        &mut self,
        params: McpServerRemoveRequest,
    ) -> Result<McpServerMutationResponse, String> {
        let _ = self.mcp_stop_server_process(&params.server_id);
        let mut items = self.storage.read_mcp_servers().map_err(Self::io_err)?;
        let mut removed = None;
        items.retain(|s| {
            if s.id == params.server_id {
                removed = Some(s.clone());
                false
            } else {
                true
            }
        });
        self.storage.write_mcp_servers(&items).map_err(Self::io_err)?;
        Ok(McpServerMutationResponse {
            ok: removed.is_some(),
            server: removed,
        })
    }

    fn mcp_servers_start(&mut self, params: McpServerStateRequest) -> Result<McpServerMutationResponse, String> {
        let items = self.storage.read_mcp_servers().map_err(Self::io_err)?;
        let record = items
            .iter()
            .find(|s| s.id == params.server_id)
            .cloned()
            .ok_or_else(|| "mcp server not found".to_string())?;
        self.mcp_spawn_server_process(&record.id, &record.command, &record.args)?;
        self.set_mcp_server_status(&params.server_id, "running")
    }

    fn mcp_servers_stop(&mut self, params: McpServerStateRequest) -> Result<McpServerMutationResponse, String> {
        self.mcp_stop_server_process(&params.server_id)?;
        self.set_mcp_server_status(&params.server_id, "stopped")
    }

    fn project_open(&mut self, params: ProjectOpenRequest) -> Result<ProjectOpenResponse, String> {
        let path = Path::new(&params.path);
        let response = ProjectOpenResponse {
            path: params.path.clone(),
            exists: path.exists(),
            is_dir: path.is_dir(),
        };
        if response.exists && response.is_dir {
            self.storage
                .write_project_state(&ProjectState {
                    open_path: Some(params.path),
                })
                .map_err(Self::io_err)?;
        }
        Ok(response)
    }

    fn project_status(&self, params: ProjectStatusRequest) -> Result<ProjectStatusResponse, String> {
        let project = self.storage.read_project_state().map_err(Self::io_err)?;
        let path = params
            .path
            .or(project.open_path)
            .unwrap_or_else(|| ".".to_string());
        let exists = Path::new(&path).exists();
        let is_dir = Path::new(&path).is_dir();
        let entry_count = if is_dir {
            std::fs::read_dir(Path::new(&path))
                .ok()
                .map(|it| it.filter_map(Result::ok).count())
                .unwrap_or(0)
        } else {
            0
        };
        Ok(ProjectStatusResponse {
            path,
            exists,
            is_dir,
            entry_count,
        })
    }

    fn audit_list(&self, params: AuditListRequest) -> Result<Vec<AuditEntry>, String> {
        let mut items = self.storage.read_audit_entries().map_err(Self::io_err)?;
        if let Some(session_id) = params.session_id {
            items.retain(|a| a.session_id.as_deref() == Some(session_id.as_str()));
        }
        items.sort_by_key(|a| a.timestamp_unix_seconds);
        items.reverse();
        if let Some(limit) = params.limit {
            items.truncate(limit);
        }
        Ok(items)
    }

    fn audit_get(&self, params: AuditGetRequest) -> Result<AuditEntry, String> {
        self.storage
            .read_audit_entries()
            .map_err(Self::io_err)?
            .into_iter()
            .find(|a| a.audit_id == params.audit_id)
            .ok_or_else(|| "audit entry not found".to_string())
    }

    fn consent_list(&self, params: ConsentListRequest) -> Result<Vec<PendingConsentRecord>, String> {
        let now = Self::now_secs();
        let mut items = self
            .storage
            .read_pending_consents()
            .map_err(Self::io_err)?
            .into_iter()
            .map(|mut x| {
                if x.record.status == "pending"
                    && x.record.expires_at_unix_seconds > 0
                    && now > x.record.expires_at_unix_seconds
                {
                    x.record.status = "expired".to_string();
                }
                x.record
            })
            .collect::<Vec<_>>();
        if let Some(status) = params.status {
            items.retain(|c| c.status == status);
        }
        if let Some(session_id) = params.session_id {
            items.retain(|c| c.session_id.as_deref() == Some(session_id.as_str()));
        }
        items.sort_by_key(|c| c.requested_at_unix_seconds);
        items.reverse();
        Ok(items)
    }

    fn consent_approve(&mut self, params: ConsentActionRequest) -> Result<ChatResponse, String> {
        self.chat_approve(ChatApproveRequest {
            consent_token: params.consent_id,
        })
    }

    fn consent_deny(&mut self, params: ConsentActionRequest) -> Result<ChatResponse, String> {
        self.chat_deny(ChatDenyRequest {
            consent_token: params.consent_id,
        })
    }

    fn tools_list(&self) -> Vec<Tool> {
        self.tool_registry.list()
    }

    fn system_health(&self) -> Result<SystemHealthResponse, String> {
        let provider_state = self.provider_state().unwrap_or_default();
        let pending_consents = self
            .storage
            .read_pending_consents()
            .map_err(Self::io_err)?
            .len();
        let _ = self.refresh_mcp_runtime_statuses();
        let mcp_servers = self.storage.read_mcp_servers().map_err(Self::io_err)?;
        let project = self.storage.read_project_state().map_err(Self::io_err)?;
        let mcp_servers_running = mcp_servers.iter().filter(|s| s.status == "running").count();
        let warnings = build_system_health_warnings(&provider_state, &project, &mcp_servers);

        Ok(SystemHealthResponse {
            ok: warnings.is_empty(),
            active_provider: provider_state.active_provider,
            provider_count: provider_state.configs.len(),
            pending_consents,
            mcp_servers_total: mcp_servers.len(),
            mcp_servers_running,
            project_path: project.open_path,
            warnings,
        })
    }
}

impl AgentService {
    fn set_mcp_server_status(
        &mut self,
        server_id: &str,
        status: &str,
    ) -> Result<McpServerMutationResponse, String> {
        let mut items = self.storage.read_mcp_servers().map_err(Self::io_err)?;
        let mut updated = None;
        for item in &mut items {
            if item.id == server_id {
                item.status = status.to_string();
                updated = Some(item.clone());
                break;
            }
        }
        self.storage.write_mcp_servers(&items).map_err(Self::io_err)?;
        Ok(McpServerMutationResponse {
            ok: updated.is_some(),
            server: updated,
        })
    }

    fn mcp_spawn_server_process(
        &mut self,
        server_id: &str,
        command: &str,
        args: &[String],
    ) -> Result<(), String> {
        let mut processes = self.mcp_processes.borrow_mut();
        if let Some(child) = processes.get_mut(server_id) {
            match child.try_wait().map_err(Self::io_err)? {
                None => return Ok(()),
                Some(_) => {
                    let _ = processes.remove(server_id);
                }
            }
        }

        let child = Command::new(command)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(Self::io_err)?;
        processes.insert(server_id.to_string(), child);
        Ok(())
    }

    fn mcp_stop_server_process(&mut self, server_id: &str) -> Result<(), String> {
        let Some(mut child) = self.mcp_processes.borrow_mut().remove(server_id) else {
            return Ok(());
        };

        if child.try_wait().map_err(Self::io_err)?.is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    fn normalize_mcp_statuses_on_startup(&self) -> Result<(), String> {
        let mut items = self.storage.read_mcp_servers().map_err(Self::io_err)?;
        let mut changed = false;
        for item in &mut items {
            if item.status == "running" {
                item.status = "stopped".to_string();
                changed = true;
            }
        }
        if changed {
            self.storage.write_mcp_servers(&items).map_err(Self::io_err)?;
        }
        Ok(())
    }

    fn refresh_mcp_runtime_statuses(&self) -> Result<(), String> {
        let mut exited_ids = Vec::new();
        {
            let mut processes = self.mcp_processes.borrow_mut();
            for (server_id, child) in processes.iter_mut() {
                if child.try_wait().map_err(Self::io_err)?.is_some() {
                    exited_ids.push(server_id.clone());
                }
            }
            for server_id in &exited_ids {
                let _ = processes.remove(server_id);
            }
        }

        if exited_ids.is_empty() {
            return Ok(());
        }

        let mut items = self.storage.read_mcp_servers().map_err(Self::io_err)?;
        let mut changed = false;
        for item in &mut items {
            if exited_ids.iter().any(|id| id == &item.id) && item.status == "running" {
                item.status = "stopped".to_string();
                changed = true;
            }
        }
        if changed {
            self.storage.write_mcp_servers(&items).map_err(Self::io_err)?;
        }
        Ok(())
    }
}

impl Drop for AgentService {
    fn drop(&mut self) {
        let ids = self
            .mcp_processes
            .borrow()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for id in ids {
            let _ = self.mcp_stop_server_process(&id);
        }
    }
}

fn provider_config_has_auth(provider_name: &str, config_json: &str) -> bool {
    let key_fields = match provider_name {
        "anthropic-stub" => &["api_key", "token"][..],
        "gemini-stub" => &["api_key", "token"][..],
        _ => &["api_key", "token"][..],
    };
    let parsed = serde_json::from_str::<serde_json::Value>(config_json).ok();
    key_fields.iter().any(|field| {
        parsed
            .as_ref()
            .and_then(|v| v.get(*field))
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }) || ["api_key_env", "token_env"].iter().any(|field| {
        parsed
            .as_ref()
            .and_then(|v| v.get(*field))
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    })
}

fn provider_config_auth_source(config_json: &str) -> Option<String> {
    let parsed = serde_json::from_str::<serde_json::Value>(config_json).ok()?;
    if ["api_key_env", "token_env"].iter().any(|field| {
        parsed
            .get(*field)
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }) {
        return Some("env".to_string());
    }
    if ["api_key", "token"].iter().any(|field| {
        parsed
            .get(*field)
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }) {
        return Some("inline".to_string());
    }
    None
}

fn redact_provider_config_json(config_json: &str) -> String {
    let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(config_json) else {
        return config_json.to_string();
    };
    let Some(obj) = parsed.as_object_mut() else {
        return config_json.to_string();
    };
    for key in ["api_key", "token", "password", "secret"] {
        if obj.contains_key(key) {
            obj.insert(
                key.to_string(),
                serde_json::Value::String("[REDACTED]".to_string()),
            );
        }
    }
    serde_json::to_string(&parsed).unwrap_or_else(|_| config_json.to_string())
}

fn build_system_health_warnings(
    provider_state: &ProviderState,
    project: &ProjectState,
    mcp_servers: &[McpServerRecord],
) -> Vec<String> {
    let mut warnings = Vec::new();

    if let Some(active) = provider_state.active_provider.as_deref() {
        let cfg = provider_state
            .configs
            .get(active)
            .cloned()
            .unwrap_or_else(|| "{}".to_string());
        if !provider_config_has_auth(active, &cfg) {
            warnings.push(format!("active provider '{}' has no configured auth", active));
        }
        if let Some(env_name) = provider_config_env_ref(&cfg) {
            if env::var(&env_name).map(|v| v.trim().is_empty()).unwrap_or(true) {
                warnings.push(format!(
                    "provider auth env var '{}' is not set or empty",
                    env_name
                ));
            }
        }
    } else {
        warnings.push("no active provider selected".to_string());
    }

    if let Some(path) = project.open_path.as_deref() {
        let p = Path::new(path);
        if !p.exists() {
            warnings.push(format!("project path does not exist: {}", path));
        } else if !p.is_dir() {
            warnings.push(format!("project path is not a directory: {}", path));
        }
    }

    if mcp_servers.iter().any(|s| s.status == "running") {
        warnings.push("MCP servers marked running are in-process only and will stop on service restart".to_string());
    }

    warnings
}

fn provider_config_env_ref(config_json: &str) -> Option<String> {
    let parsed = serde_json::from_str::<serde_json::Value>(config_json).ok()?;
    for key in ["api_key_env", "token_env"] {
        if let Some(name) = parsed.get(key).and_then(|v| v.as_str()) {
            if !name.trim().is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn build_consent_request(
    proposed_actions: &[ActionEvent],
    expires_at_unix_seconds: Option<u64>,
    ttl_seconds: Option<u64>,
) -> ConsentRequest {
    let pending: Vec<&ActionEvent> = proposed_actions
        .iter()
        .filter(|evt| evt.status == "consent_required")
        .collect();
    let requires_extra_confirmation_click = pending.iter().any(|evt| {
        matches!(evt.capability_tier.as_str(), "LocalActions" | "SystemActions")
    });

    let mut risk_factors = Vec::new();
    if pending.iter().any(|evt| evt.capability_tier == "LocalActions") {
        risk_factors.push("local_device_action".to_string());
    }
    if pending.iter().any(|evt| evt.capability_tier == "SystemActions") {
        risk_factors.push("system_level_action".to_string());
    }
    if pending.len() > 1 {
        risk_factors.push("multiple_actions_requested".to_string());
    }
    if pending
        .iter()
        .any(|evt| evt.arguments_preview.as_deref().map(|s| !s.is_empty()).unwrap_or(false))
    {
        risk_factors.push("external_arguments_present".to_string());
    }

    let human_summary = if pending.is_empty() {
        "No consent-required actions pending.".to_string()
    } else if pending.len() == 1 {
        format!(
            "Approve execution of '{}' once for this exact request.",
            pending[0].tool_name
        )
    } else {
        format!(
            "Approve execution of {} actions once for this exact request.",
            pending.len()
        )
    };

    ConsentRequest {
        scope: "once_exact_request".to_string(),
        human_summary,
        risk_factors,
        requires_extra_confirmation_click,
        expires_at_unix_seconds,
        ttl_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc::jsonrpc::{Id, Request};
    use ipc::{
        AuditListRequest, JsonRpcServer, McpServerAddRequest, McpServerStateRequest,
        ProjectOpenRequest,
    };
    use std::fs;
    #[cfg(unix)]
    use std::thread;
    #[cfg(unix)]
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn consent_lifecycle_approve_continues_and_records_audit() {
        let dir = tempdir().expect("tempdir");
        let service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        let mut server = JsonRpcServer::new(service);

        let chat_req = ipc::ChatRequest {
            session_id: None,
            messages: vec![ipc::ChatMessage {
                role: "user".to_string(),
                content: "tool:activate Browser".to_string(),
            }],
            provider_config: ipc::ProviderConfig {
                provider_name: "openai-stub".to_string(),
                model: None,
            },
            mode: ipc::ChatMode::RequireConfirmation,
        };
        let raw = server.handle(Request::new(
            Id::Number(1),
            "chat.request",
            serde_json::to_string(&chat_req).expect("serialize"),
        ));
        let first: ipc::ChatResponse = serde_json::from_str(
            raw.result_json.as_deref().expect("result"),
        )
        .expect("chat response");
        let consent_id = first.consent_token.clone().expect("consent token");
        assert!(first
            .proposed_actions
            .iter()
            .any(|a| a.status == "consent_required"));

        let raw2 = server.handle(Request::new(
            Id::Number(2),
            "chat.approve",
            serde_json::to_string(&ipc::ChatApproveRequest {
                consent_token: consent_id,
            })
            .expect("serialize"),
        ));
        let second: ipc::ChatResponse = serde_json::from_str(
            raw2.result_json.as_deref().expect("result"),
        )
        .expect("chat response");
        assert!(second
            .executed_action_events
            .iter()
            .any(|a| a.status == "executed"));

        let audits = server
            .service()
            .audit_list(AuditListRequest {
                session_id: None,
                limit: Some(10),
            })
            .expect("audit list");
        assert!(audits.len() >= 2);
    }

    #[test]
    fn consent_approve_replay_returns_explicit_not_pending_error() {
        let dir = tempdir().expect("tempdir");
        let service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        let mut server = JsonRpcServer::new(service);

        let chat_req = ipc::ChatRequest {
            session_id: None,
            messages: vec![ipc::ChatMessage {
                role: "user".to_string(),
                content: "tool:activate Browser".to_string(),
            }],
            provider_config: ipc::ProviderConfig {
                provider_name: "openai-stub".to_string(),
                model: None,
            },
            mode: ipc::ChatMode::RequireConfirmation,
        };
        let first = server.handle(Request::new(
            Id::Number(1),
            "chat.request",
            serde_json::to_string(&chat_req).expect("serialize"),
        ));
        let first: ipc::ChatResponse =
            serde_json::from_str(first.result_json.as_deref().expect("result")).expect("chat response");
        let consent_id = first.consent_token.expect("consent token");

        let _approved = server.handle(Request::new(
            Id::Number(2),
            "chat.approve",
            serde_json::to_string(&ipc::ChatApproveRequest {
                consent_token: consent_id.clone(),
            })
            .expect("serialize"),
        ));

        let replay = server.handle(Request::new(
            Id::Number(3),
            "chat.approve",
            serde_json::to_string(&ipc::ChatApproveRequest {
                consent_token: consent_id,
            })
            .expect("serialize"),
        ));
        let err = replay.error.expect("json-rpc error");
        assert!(err.message.contains("consent_not_pending:approved"));
    }

    #[test]
    fn consent_approve_expired_returns_explicit_error() {
        let dir = tempdir().expect("tempdir");
        let service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        let mut server = JsonRpcServer::new(service);

        let first = server.handle(Request::new(
            Id::Number(1),
            "chat.request",
            serde_json::to_string(&ipc::ChatRequest {
                session_id: None,
                messages: vec![ipc::ChatMessage {
                    role: "user".to_string(),
                    content: "tool:activate Browser".to_string(),
                }],
                provider_config: ipc::ProviderConfig {
                    provider_name: "openai-stub".to_string(),
                    model: None,
                },
                mode: ipc::ChatMode::RequireConfirmation,
            })
            .expect("serialize"),
        ));
        let first: ipc::ChatResponse =
            serde_json::from_str(first.result_json.as_deref().expect("result")).expect("chat response");
        let consent_id = first.consent_token.expect("consent token");

        let mut pending = server
            .service()
            .storage
            .read_pending_consents()
            .expect("read pending");
        let target = pending
            .iter_mut()
            .find(|p| p.record.consent_id == consent_id)
            .expect("pending record");
        target.record.expires_at_unix_seconds = 1;
        server
            .service()
            .storage
            .write_pending_consents(&pending)
            .expect("write pending");

        let expired = server.handle(Request::new(
            Id::Number(2),
            "chat.approve",
            serde_json::to_string(&ipc::ChatApproveRequest { consent_token: consent_id }).expect("serialize"),
        ));
        let err = expired.error.expect("json-rpc error");
        assert!(err.message.contains("consent_expired"));
    }

    #[test]
    fn file_read_text_uses_project_scope() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("notes.txt"), "hello project\n").expect("write file");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        service
            .project_open(ProjectOpenRequest {
                path: dir.path().display().to_string(),
            })
            .expect("project open");

        let response = service.chat_request(ipc::ChatRequest {
            session_id: None,
            messages: vec![ipc::ChatMessage {
                role: "user".to_string(),
                content: "tool:cat notes.txt".to_string(),
            }],
            provider_config: ipc::ProviderConfig {
                provider_name: "openai-stub".to_string(),
                model: None,
            },
            mode: ipc::ChatMode::BestEffort,
        });

        assert!(response
            .executed_action_events
            .iter()
            .any(|a| a.tool_name == "file.read_text" && a.status == "executed"));
    }

    #[test]
    fn file_write_text_requires_consent_in_best_effort() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        service
            .project_open(ProjectOpenRequest {
                path: dir.path().display().to_string(),
            })
            .expect("project open");

        let response = service.chat_request(ipc::ChatRequest {
            session_id: None,
            messages: vec![ipc::ChatMessage {
                role: "user".to_string(),
                content: "tool:write notes/out.txt :: hello".to_string(),
            }],
            provider_config: ipc::ProviderConfig {
                provider_name: "openai-stub".to_string(),
                model: None,
            },
            mode: ipc::ChatMode::BestEffort,
        });

        assert!(response
            .proposed_actions
            .iter()
            .any(|a| a.tool_name == "file.write_text" && a.status == "consent_required"));
        assert!(response.consent_token.is_some());
    }

    #[test]
    fn file_append_and_mkdir_require_consent_in_best_effort() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        service
            .project_open(ProjectOpenRequest {
                path: dir.path().display().to_string(),
            })
            .expect("project open");

        for prompt in ["tool:append notes/out.txt :: hello", "tool:mkdir notes/subdir"] {
            let response = service.chat_request(ipc::ChatRequest {
                session_id: None,
                messages: vec![ipc::ChatMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                }],
                provider_config: ipc::ProviderConfig {
                    provider_name: "openai-stub".to_string(),
                    model: None,
                },
                mode: ipc::ChatMode::BestEffort,
            });
            assert!(response
                .proposed_actions
                .iter()
                .any(|a| a.status == "consent_required"));
            assert!(response.consent_token.is_some());
        }
    }

    #[test]
    fn session_history_persists_assistant_response() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        let session = service
            .sessions_create(SessionCreateRequest {
                title: Some("Test Session".to_string()),
            })
            .expect("create session");

        let response = service.chat_request(ipc::ChatRequest {
            session_id: Some(session.id.clone()),
            messages: vec![ipc::ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            provider_config: ipc::ProviderConfig {
                provider_name: "openai-stub".to_string(),
                model: None,
            },
            mode: ipc::ChatMode::BestEffort,
        });

        let stored = service
            .sessions_get(SessionGetRequest {
                session_id: session.id,
            })
            .expect("get session");
        assert_eq!(stored.messages.len(), 2);
        assert_eq!(stored.messages[0].role, "user");
        assert_eq!(stored.messages[0].content, "hello");
        assert_eq!(stored.messages[1].role, "assistant");
        assert_eq!(stored.messages[1].content, response.final_text);
    }

    #[cfg(unix)]
    #[test]
    fn mcp_server_start_stop_spawns_process_and_updates_status() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());

        let added = service
            .mcp_servers_add(McpServerAddRequest {
                name: "sleepy".to_string(),
                command: "/bin/sh".to_string(),
                args: vec!["-c".to_string(), "sleep 30".to_string()],
            })
            .expect("add mcp server");
        let server = added.server.expect("server record");

        let started = service
            .mcp_servers_start(McpServerStateRequest {
                server_id: server.id.clone(),
            })
            .expect("start server");
        assert!(started.ok);
        assert_eq!(started.server.as_ref().map(|s| s.status.as_str()), Some("running"));

        let stopped = service
            .mcp_servers_stop(McpServerStateRequest {
                server_id: server.id,
            })
            .expect("stop server");
        assert!(stopped.ok);
        assert_eq!(stopped.server.as_ref().map(|s| s.status.as_str()), Some("stopped"));
    }

    #[cfg(unix)]
    #[test]
    fn mcp_servers_list_marks_exited_processes_stopped() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());

        let added = service
            .mcp_servers_add(McpServerAddRequest {
                name: "short-lived".to_string(),
                command: "/bin/sh".to_string(),
                args: vec!["-c".to_string(), "sleep 0.1".to_string()],
            })
            .expect("add mcp server");
        let server = added.server.expect("server record");

        service
            .mcp_servers_start(McpServerStateRequest {
                server_id: server.id.clone(),
            })
            .expect("start server");

        thread::sleep(Duration::from_millis(250));

        let listed = service.mcp_servers_list().expect("list servers");
        let record = listed
            .iter()
            .find(|s| s.id == server.id)
            .expect("server in list");
        assert_eq!(record.status, "stopped");
    }

    #[test]
    fn startup_normalizes_persisted_running_mcp_statuses() {
        let dir = tempdir().expect("tempdir");
        {
            let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
            let added = service
                .mcp_servers_add(McpServerAddRequest {
                    name: "persisted".to_string(),
                    command: "echo".to_string(),
                    args: vec!["hi".to_string()],
                })
                .expect("add mcp server");
            let id = added.server.expect("server").id;
            let mut items = service.storage.read_mcp_servers().expect("read mcp");
            let item = items.iter_mut().find(|s| s.id == id).expect("server exists");
            item.status = "running".to_string();
            service.storage.write_mcp_servers(&items).expect("write mcp");
        }

        let service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        let listed = service.mcp_servers_list().expect("list servers");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, "stopped");
    }

    #[test]
    fn providers_list_recognizes_env_based_auth_config() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        service
            .providers_config_set(ProviderConfigSetRequest {
                provider_name: "openai".to_string(),
                config_json: r#"{"api_key_env":"OPENAI_API_KEY"}"#.to_string(),
            })
            .expect("set provider config");

        let providers = service.providers_list().expect("providers list");
        let openai = providers.iter().find(|p| p.name == "openai").expect("openai provider");
        assert!(openai.has_auth);
        assert_eq!(openai.config_summary, "configured (env)");
    }

    #[test]
    fn providers_list_includes_custom_provider_aliases() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        service
            .providers_config_set(ProviderConfigSetRequest {
                provider_name: "ollama-local".to_string(),
                config_json: r#"{"api_key_env":"OLLAMA_TOKEN"}"#.to_string(),
            })
            .expect("set custom provider config");

        let providers = service.providers_list().expect("providers list");
        let custom = providers
            .iter()
            .find(|p| p.name == "ollama-local")
            .expect("custom provider present");
        assert!(custom.has_auth);
        assert_eq!(custom.config_summary, "configured (env)");

        let active = service
            .providers_set(ProvidersSetRequest {
                provider_name: "ollama-local".to_string(),
            })
            .expect("set custom provider active");
        assert_eq!(active.name, "ollama-local");
        assert!(active.is_active);
    }

    #[test]
    fn providers_config_get_redacts_inline_secret_values() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        service
            .providers_config_set(ProviderConfigSetRequest {
                provider_name: "openai".to_string(),
                config_json: r#"{"api_key":"sk-test","api_key_env":"OPENAI_API_KEY"}"#.to_string(),
            })
            .expect("set provider config");

        let record = service
            .providers_config_get(ProviderConfigGetRequest {
                provider_name: Some("openai".to_string()),
            })
            .expect("config get");
        assert!(record.config_json.contains("\"api_key\":\"[REDACTED]\""));
        assert!(record.config_json.contains("\"api_key_env\":\"OPENAI_API_KEY\""));
        assert!(!record.config_json.contains("sk-test"));
    }

    #[test]
    fn system_health_reports_basic_runtime_state() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        let _ = service.providers_config_set(ProviderConfigSetRequest {
            provider_name: "openai".to_string(),
            config_json: r#"{"api_key":"test-key"}"#.to_string(),
        });
        let _ = service.providers_set(ProvidersSetRequest {
            provider_name: "openai".to_string(),
        });
        let _ = service.project_open(ProjectOpenRequest {
            path: dir.path().display().to_string(),
        });

        let health = service.system_health().expect("system health");
        assert!(health.ok);
        assert_eq!(health.active_provider.as_deref(), Some("openai"));
        assert_eq!(health.provider_count, 1);
        assert_eq!(health.mcp_servers_total, 0);
        assert_eq!(health.mcp_servers_running, 0);
        assert_eq!(health.pending_consents, 0);
        assert!(health.project_path.is_some());
        assert!(health.warnings.is_empty());
    }

    #[test]
    fn system_health_warns_when_provider_env_ref_missing() {
        let dir = tempdir().expect("tempdir");
        let mut service = AgentService::new_for_platform_with_storage_dir("test", dir.path());
        service
            .providers_config_set(ProviderConfigSetRequest {
                provider_name: "openai".to_string(),
                config_json: r#"{"api_key_env":"CMND_N_CTRL_TEST_MISSING_KEY"}"#.to_string(),
            })
            .expect("config set");
        service
            .providers_set(ProvidersSetRequest {
                provider_name: "openai".to_string(),
            })
            .expect("provider set");

        let health = service.system_health().expect("system health");
        assert!(!health.ok);
        assert!(health
            .warnings
            .iter()
            .any(|w| w.contains("CMND_N_CTRL_TEST_MISSING_KEY")));
    }
}
