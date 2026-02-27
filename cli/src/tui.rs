use agent::AgentService;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ipc::jsonrpc::{Id, Request};
use ipc::{AuditEntry, ChatMessage, ChatMode, ChatRequest, ChatResponse, ExecutionFeedItem, JsonRpcClient, PendingConsentRecord, Session, SessionSummary};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::io::stdout;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPane {
    Sessions,
    Chat,
    Consents,
    Audit,
}

impl FocusPane {
    fn next(self) -> Self {
        match self {
            Self::Sessions => Self::Chat,
            Self::Chat => Self::Consents,
            Self::Consents => Self::Audit,
            Self::Audit => Self::Sessions,
        }
    }
}

struct TuiApp {
    sessions: Vec<SessionSummary>,
    selected_session: usize,
    session_detail: Option<Session>,
    consents: Vec<PendingConsentRecord>,
    selected_consent: usize,
    audits: Vec<AuditEntry>,
    selected_audit: usize,
    chat_input: String,
    status: String,
    focus: FocusPane,
    require_confirmation: bool,
    provider_name: String,
    last_chat_response: Option<ChatResponse>,
    feed: Vec<ExecutionFeedItem>,
    selected_execution: usize,
    show_execution_details: bool,
}

fn pane_title(base: &str, focused: bool) -> String {
    if focused {
        format!("[{}]", base)
    } else {
        base.to_string()
    }
}

fn focused_block<'a>(title: String, focused: bool) -> Block<'a> {
    let border = if focused { Color::Gray } else { Color::DarkGray };
    Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(border))
        .title(title)
}

impl TuiApp {
    fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected_session: 0,
            session_detail: None,
            consents: Vec::new(),
            selected_consent: 0,
            audits: Vec::new(),
            selected_audit: 0,
            chat_input: String::new(),
            status: "Ready".to_string(),
            focus: FocusPane::Chat,
            require_confirmation: true,
            provider_name: "openai-stub".to_string(),
            last_chat_response: None,
            feed: Vec::new(),
            selected_execution: 0,
            show_execution_details: false,
        }
    }

    fn current_session_id(&self) -> Option<String> {
        self.sessions.get(self.selected_session).map(|s| s.id.clone())
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }
}

pub fn run(client: &mut JsonRpcClient<AgentService>) -> Result<(), String> {
    enable_raw_mode().map_err(|e| e.to_string())?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen).map_err(|e| e.to_string())?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;
    terminal.clear().map_err(|e| e.to_string())?;

    let mut app = TuiApp::new();
    let result = run_loop(&mut terminal, client, &mut app);

    disable_raw_mode().map_err(|e| e.to_string())?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(|e| e.to_string())?;
    terminal.show_cursor().map_err(|e| e.to_string())?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    client: &mut JsonRpcClient<AgentService>,
    app: &mut TuiApp,
) -> Result<(), String> {
    refresh_all(client, app)?;
    loop {
        terminal.draw(|f| render(f, app)).map_err(|e| e.to_string())?;
        if event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
            if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Tab => app.focus = app.focus.next(),
                    KeyCode::Char('r') => {
                        refresh_all(client, app)?;
                    }
                    KeyCode::Char('c') => {
                        app.require_confirmation = !app.require_confirmation;
                        app.set_status(format!(
                            "Require confirmation: {}",
                            if app.require_confirmation { "on" } else { "off" }
                        ));
                    }
                    KeyCode::Char('n') => {
                        create_session(client, app)?;
                    }
                    KeyCode::Char('x') if app.focus == FocusPane::Sessions => {
                        delete_selected_session(client, app)?;
                    }
                    KeyCode::Char('a') if app.focus == FocusPane::Consents => {
                        approve_selected_consent(client, app)?;
                    }
                    KeyCode::Char('d') if app.focus == FocusPane::Consents => {
                        deny_selected_consent(client, app)?;
                    }
                    KeyCode::Down | KeyCode::Char('j') => move_selection(app, 1),
                    KeyCode::Up | KeyCode::Char('k') => move_selection(app, -1),
                    KeyCode::Char('v') if app.focus == FocusPane::Chat => {
                        app.show_execution_details = !app.show_execution_details;
                        app.set_status(if app.show_execution_details {
                            "Execution details expanded"
                        } else {
                            "Execution details collapsed"
                        });
                    }
                    KeyCode::Enter => match app.focus {
                        FocusPane::Sessions => load_selected_session(client, app)?,
                        FocusPane::Chat => send_chat(client, app)?,
                        FocusPane::Consents => approve_selected_consent(client, app)?,
                        FocusPane::Audit => {}
                    },
                    KeyCode::Backspace if app.focus == FocusPane::Chat => {
                        app.chat_input.pop();
                    }
                    KeyCode::Esc if app.focus == FocusPane::Chat => {
                        app.chat_input.clear();
                    }
                    KeyCode::Char(ch)
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        if app.focus != FocusPane::Chat {
                            app.focus = FocusPane::Chat;
                        }
                        app.chat_input.push(ch);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn render(frame: &mut Frame, app: &TuiApp) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let columns = match app.focus {
        FocusPane::Chat => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(0), Constraint::Min(10), Constraint::Length(0)])
            .split(outer[0]),
        FocusPane::Sessions => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(28), Constraint::Percentage(72), Constraint::Length(0)])
            .split(outer[0]),
        FocusPane::Consents | FocusPane::Audit => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(0), Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(outer[0]),
    };

    if columns[0].width > 0 {
        render_sessions(frame, columns[0], app);
    }
    render_chat(frame, columns[1], app);
    if columns[2].width > 0 {
        render_right(frame, columns[2], app);
    }

    render_input(frame, outer[1], app);
    render_status(frame, outer[2], app);
}

fn render_sessions(frame: &mut Frame, area: ratatui::layout::Rect, app: &TuiApp) {
    let items = if app.sessions.is_empty() {
        vec![ListItem::new("(no sessions)")]
    } else {
        app.sessions
            .iter()
            .map(|s| ListItem::new(format!("{} ({})", s.id, s.message_count)))
            .collect()
    };
    let mut state = ListState::default().with_selected(if app.sessions.is_empty() {
        None
    } else {
        Some(app.selected_session.min(app.sessions.len() - 1))
    });
    let title = pane_title("Sessions", app.focus == FocusPane::Sessions);
    let list = List::new(items)
        .block(focused_block(title, app.focus == FocusPane::Sessions))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_chat(frame: &mut Frame, area: ratatui::layout::Rect, app: &TuiApp) {
    let mut lines = Vec::<Line>::new();
    if !app.feed.is_empty() {
        for (idx, entry) in app.feed.iter().enumerate().rev() {
            let marker = if idx == app.selected_execution && app.focus == FocusPane::Chat {
                ">"
            } else {
                " "
            };
            let row = format!(
                "{marker} [{}] {}",
                compact_status(&entry.status),
                entry
                    .user_prompt
                    .as_deref()
                    .map(|p| truncate_inline(p, 56))
                    .unwrap_or_else(|| "(no prompt)".to_string())
            );
            lines.push(Line::from(row));
            if idx == app.selected_execution && app.show_execution_details {
                lines.push(Line::from(format!("   assistant> {}", entry.assistant_text)));
                if let Some(session_id) = &entry.session_id {
                    lines.push(Line::from(format!("   session> {}", session_id)));
                }
                if let Some(consent) = &entry.consent_request {
                    lines.push(Line::from(format!("   consent?> {}", consent.human_summary)));
                }
                if !entry.proposed_actions.is_empty() || !entry.executed_action_events.is_empty() {
                    let proposed = entry
                        .proposed_actions
                        .iter()
                        .map(|evt| format!("{}:{}", evt.tool_name, evt.status))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let executed = entry
                        .executed_action_events
                        .iter()
                        .map(|evt| format!("{}:{}", evt.tool_name, evt.status))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(Line::from(format!("   tools> p=[{}] e=[{}]", proposed, executed)));
                }
            }
        }
    } else {
        lines.push(Line::from("system> no executions yet; send a natural-language request"));
    }
    let chat = Paragraph::new(lines)
        .block(focused_block(
            pane_title("Feed (v details)", app.focus == FocusPane::Chat),
            app.focus == FocusPane::Chat,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(chat, area);
}

fn render_right(frame: &mut Frame, area: ratatui::layout::Rect, app: &TuiApp) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let consent_items = if app.consents.is_empty() {
        vec![ListItem::new("(no pending consents)")]
    } else {
        app.consents
            .iter()
            .map(|c| {
                let ttl = if c.expires_at_unix_seconds > 0 {
                    format!(" exp={}", c.expires_at_unix_seconds)
                } else {
                    String::new()
                };
                let label = format!("{} [{}] {}{}", c.consent_id, c.capability_tier, c.tool_name, ttl);
                ListItem::new(label)
            })
            .collect()
    };
    let mut consent_state = ListState::default().with_selected(if app.consents.is_empty() {
        None
    } else {
        Some(app.selected_consent.min(app.consents.len() - 1))
    });
    let consent_title = pane_title("Consent", app.focus == FocusPane::Consents);
    let consent_list = List::new(consent_items)
        .block(focused_block(consent_title, app.focus == FocusPane::Consents))
        .highlight_style(Style::default().fg(Color::Yellow));
    frame.render_stateful_widget(consent_list, rows[0], &mut consent_state);

    let audit_lines = if app.audits.is_empty() {
        vec![Line::from("(no audit entries)")]
    } else {
        app.audits
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let marker = if i == app.selected_audit && app.focus == FocusPane::Audit {
                    ">"
                } else {
                    " "
                };
                Line::from(format!("{marker} {} {}", a.audit_id, a.provider))
            })
            .collect()
    };
    let audit = Paragraph::new(audit_lines)
        .block(focused_block(
            pane_title("Audit", app.focus == FocusPane::Audit),
            app.focus == FocusPane::Audit,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(audit, rows[1]);
}

fn render_input(frame: &mut Frame, area: ratatui::layout::Rect, app: &TuiApp) {
    let title = format!(
        "input [{}] {} {}",
        if app.require_confirmation {
            "confirm"
        } else {
            "best"
        },
        app.provider_name,
        app.current_session_id().unwrap_or_else(|| "(none)".to_string())
    );
    let text = if app.chat_input.is_empty() {
        "-> ".to_string()
    } else {
        format!("-> {}", app.chat_input)
    };
    let input = Paragraph::new(text)
        .style(Style::default().bg(Color::Rgb(52, 56, 64)).fg(Color::White))
        .block(focused_block(title, app.focus == FocusPane::Chat))
        .wrap(Wrap { trim: false });
    frame.render_widget(input, area);
}

fn render_status(frame: &mut Frame, area: ratatui::layout::Rect, app: &TuiApp) {
    let pane = match app.focus {
        FocusPane::Sessions => "sessions",
        FocusPane::Chat => "feed",
        FocusPane::Consents => "consent",
        FocusPane::Audit => "audit",
    };
    let status = Paragraph::new(format!("{pane} | {}", app.status))
        .style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM));
    frame.render_widget(status, area);
}

fn move_selection(app: &mut TuiApp, delta: isize) {
    match app.focus {
        FocusPane::Sessions => {
            if app.sessions.is_empty() {
                return;
            }
            let len = app.sessions.len() as isize;
            let idx = (app.selected_session as isize + delta).clamp(0, len - 1);
            app.selected_session = idx as usize;
        }
        FocusPane::Consents => {
            if app.consents.is_empty() {
                return;
            }
            let len = app.consents.len() as isize;
            let idx = (app.selected_consent as isize + delta).clamp(0, len - 1);
            app.selected_consent = idx as usize;
        }
        FocusPane::Audit => {
            if app.audits.is_empty() {
                return;
            }
            let len = app.audits.len() as isize;
            let idx = (app.selected_audit as isize + delta).clamp(0, len - 1);
            app.selected_audit = idx as usize;
        }
        FocusPane::Chat => {
            if app.feed.is_empty() {
                return;
            }
            let len = app.feed.len() as isize;
            let idx = (app.selected_execution as isize + delta).clamp(0, len - 1);
            app.selected_execution = idx as usize;
        }
    }
}

fn local_call<T: DeserializeOwned>(
    client: &mut JsonRpcClient<AgentService>,
    method: &str,
    params: serde_json::Value,
) -> Result<T, String> {
    let resp = client.call_raw(Request::new(Id::Number(1), method.to_string(), params.to_string()));
    if let Some(err) = resp.error {
        return Err(format!("json-rpc {}: {}", err.code, err.message));
    }
    let payload = resp
        .result_json
        .ok_or_else(|| "missing result payload".to_string())?;
    serde_json::from_str::<T>(&payload).map_err(|e| format!("result parse error: {e}"))
}

fn refresh_all(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    app.sessions = local_call(client, "sessions.list", json!({}))?;
    if app.selected_session >= app.sessions.len() && !app.sessions.is_empty() {
        app.selected_session = app.sessions.len() - 1;
    }
    load_selected_session(client, app)?;
    refresh_consents(client, app)?;
    refresh_audit(client, app)?;
    app.set_status("Refreshed");
    Ok(())
}

fn load_selected_session(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    if let Some(session_id) = app.current_session_id() {
        let session: Session = local_call(client, "sessions.get", json!({ "session_id": session_id }))?;
        app.session_detail = Some(session);
        app.set_status("Loaded session");
    } else {
        app.session_detail = None;
    }
    Ok(())
}

fn refresh_consents(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    app.consents = local_call(
        client,
        "consent.list",
        json!({
            "status": "pending",
            "session_id": app.current_session_id(),
        }),
    )?;
    if app.selected_consent >= app.consents.len() && !app.consents.is_empty() {
        app.selected_consent = app.consents.len() - 1;
    }
    Ok(())
}

fn refresh_audit(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    app.audits = local_call(
        client,
        "audit.list",
        json!({
            "session_id": app.current_session_id(),
            "limit": 20
        }),
    )?;
    if app.selected_audit >= app.audits.len() && !app.audits.is_empty() {
        app.selected_audit = app.audits.len() - 1;
    }
    Ok(())
}

fn create_session(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    let session: Session = local_call(client, "sessions.create", json!({ "title": null }))?;
    app.set_status(format!("Created {}", session.id));
    refresh_all(client, app)?;
    if let Some(idx) = app.sessions.iter().position(|s| s.id == session.id) {
        app.selected_session = idx;
        load_selected_session(client, app)?;
    }
    Ok(())
}

fn delete_selected_session(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    let Some(session_id) = app.current_session_id() else {
        app.set_status("No session selected");
        return Ok(());
    };
    let _: serde_json::Value = local_call(client, "sessions.delete", json!({ "session_id": session_id.clone() }))?;
    app.set_status(format!("Deleted {}", session_id));
    if app.selected_session > 0 {
        app.selected_session -= 1;
    }
    refresh_all(client, app)
}

fn send_chat(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    if app.chat_input.trim().is_empty() {
        app.set_status("Input is empty");
        return Ok(());
    }
    if app.chat_input.to_ascii_lowercase().contains("tool:") {
        app.set_status("Use natural language only (explicit tool: syntax disabled)");
        return Ok(());
    }
    if app.current_session_id().is_none() {
        create_session(client, app)?;
    }
    let prompt = std::mem::take(&mut app.chat_input);
    let request = ChatRequest {
        session_id: app.current_session_id(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.clone(),
        }],
        provider_config: ipc::ProviderConfig {
            provider_name: app.provider_name.clone(),
            model: None,
            config_json: None,
        },
        mode: if app.require_confirmation {
            ChatMode::RequireConfirmation
        } else {
            ChatMode::BestEffort
        },
    };
    let response: ChatResponse = local_call(client, "chat.request", serde_json::to_value(request).unwrap_or(json!({})))?;
    app.last_chat_response = Some(response.clone());
    app.feed.push(response.to_execution_feed_item(Some(prompt)));
    app.selected_execution = app.feed.len().saturating_sub(1);
    if let Some(sid) = response.session_id.as_ref() {
        if let Some(idx) = app.sessions.iter().position(|s| &s.id == sid) {
            app.selected_session = idx;
        }
    }
    load_selected_session(client, app)?;
    refresh_consents(client, app)?;
    refresh_audit(client, app)?;
    app.set_status(response.final_text);
    Ok(())
}

fn approve_selected_consent(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    let Some(consent) = app.consents.get(app.selected_consent).cloned() else {
        app.set_status("No pending consent selected");
        return Ok(());
    };
    let response: ChatResponse = match local_call(client, "consent.approve", json!({ "consent_id": consent.consent_id })) {
        Ok(r) => r,
        Err(err) => {
            refresh_consents(client, app)?;
            app.set_status(format!("Approve failed: {}", humanize_consent_error(&err)));
            return Ok(());
        }
    };
    app.last_chat_response = Some(response.clone());
    app.feed.push(response.to_execution_feed_item(None));
    app.selected_execution = app.feed.len().saturating_sub(1);
    load_selected_session(client, app)?;
    refresh_consents(client, app)?;
    refresh_audit(client, app)?;
    app.set_status("Consent approved");
    Ok(())
}

fn deny_selected_consent(client: &mut JsonRpcClient<AgentService>, app: &mut TuiApp) -> Result<(), String> {
    let Some(consent) = app.consents.get(app.selected_consent).cloned() else {
        app.set_status("No pending consent selected");
        return Ok(());
    };
    let response: ChatResponse = match local_call(client, "consent.deny", json!({ "consent_id": consent.consent_id })) {
        Ok(r) => r,
        Err(err) => {
            refresh_consents(client, app)?;
            app.set_status(format!("Deny failed: {}", humanize_consent_error(&err)));
            return Ok(());
        }
    };
    app.last_chat_response = Some(response.clone());
    app.feed.push(response.to_execution_feed_item(None));
    app.selected_execution = app.feed.len().saturating_sub(1);
    refresh_consents(client, app)?;
    refresh_audit(client, app)?;
    app.set_status("Consent denied");
    Ok(())
}

fn truncate_inline(input: &str, max: usize) -> String {
    let mut chars = input.chars();
    let out: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{out}...")
    } else {
        out
    }
}

fn compact_status(status: &str) -> &str {
    match status {
        "completed" => "done",
        "denied" => "blocked",
        "error" => "failed",
        other => other,
    }
}

fn humanize_consent_error(err: &str) -> String {
    if err.contains("consent_expired") {
        "consent request expired".to_string()
    } else if err.contains("consent_not_pending:approved") {
        "consent already approved".to_string()
    } else if err.contains("consent_not_pending:denied") {
        "consent already denied".to_string()
    } else if err.contains("consent_not_pending:expired") {
        "consent request expired".to_string()
    } else if err.contains("consent_not_found") {
        "consent request not found".to_string()
    } else {
        err.to_string()
    }
}
