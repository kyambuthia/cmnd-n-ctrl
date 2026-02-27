const promptEl = document.querySelector('#prompt');
const currentActionLabelEl = document.querySelector('#currentActionLabel');
const currentActionBodyEl = document.querySelector('#currentActionBody');
const currentActionMetaEl = document.querySelector('#currentActionMeta');
const activityHistoryEl = document.querySelector('#activityHistory');
const historySearchEl = document.querySelector('#historySearch');
const auditEl = document.querySelector('#audit');
const workspaceStateEl = document.querySelector('#workspaceState');
const actionsEl = document.querySelector('#actions');
const actionChipsEl = document.querySelector('#actionChips');
const rawEl = document.querySelector('#raw');
const statusEl = document.querySelector('#status');
const shellEl = document.querySelector('.shell');
const transportBadgeEl = document.querySelector('#transportBadge');
const debugToggleBtn = document.querySelector('#debugToggle');
const requireConfirmationEl = document.querySelector('#requireConfirmation');
const sendBtn = document.querySelector('#send');
const listToolsBtn = document.querySelector('#listTools');
const openProjectBtn = document.querySelector('#openProject');
const projectStatusBtn = document.querySelector('#projectStatus');
const projectPathEl = document.querySelector('#projectPath');
const newSessionBtn = document.querySelector('#newSession');
const listSessionsBtn = document.querySelector('#listSessions');
const listProvidersBtn = document.querySelector('#listProviders');
const listConsentsBtn = document.querySelector('#listConsents');
const listAuditBtn = document.querySelector('#listAudit');
const sessionIdEl = document.querySelector('#sessionId');
const clearViewBtn = document.querySelector('#clearView');
const consentCardEl = document.querySelector('#consentCard');
const consentSummaryEl = document.querySelector('#consentSummary');
const consentRequestedEl = document.querySelector('#consentRequested');
const consentDetailsEl = document.querySelector('#consentDetails');
const consentScopeEl = document.querySelector('#consentScope');
const approveConsentBtn = document.querySelector('#approveConsent');
const denyConsentBtn = document.querySelector('#denyConsent');
const presetButtons = Array.from(document.querySelectorAll('.prompt-preset'));

const JSONRPC_URL = 'http://127.0.0.1:7777/jsonrpc';
let transport = null;
let lastChatContext = null;
let pendingConsent = null;
let consentApprovalArmed = false;
let pendingConsentFingerprint = null;
let pendingConsentToken = null;
let pendingConsentMeta = null;
let historyFilter = '';

function nowLabel() {
  return new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function setStatus(message) {
  statusEl.innerHTML = `<span class="dot"></span>${escapeHtml(message)}`;
}

function setRaw(payload) {
  rawEl.textContent = typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2);
}

function setWorkspaceState(payload) {
  if (!workspaceStateEl) return;
  workspaceStateEl.textContent =
    typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2);
}

function setCurrentAction(kind, label, body, meta = []) {
  currentActionLabelEl.textContent = label;
  currentActionBodyEl.textContent = body;
  currentActionMetaEl.innerHTML = '';

  const metaItems = Array.isArray(meta) && meta.length ? meta : ['idle'];
  for (const item of metaItems) {
    const chip = document.createElement('span');
    chip.className = 'chip';
    chip.textContent = item;
    currentActionMetaEl.appendChild(chip);
  }
}

function clearHistory() {
  activityHistoryEl.innerHTML = `
    <div class="history-item">
      <div class="meta">Ready • ${nowLabel()}</div>
      <div class="text">No action history yet.</div>
    </div>
  `;
}

function applyHistoryFilter() {
  const q = historyFilter.trim().toLowerCase();
  const items = Array.from(activityHistoryEl.querySelectorAll('.history-item'));
  for (const item of items) {
    const haystack = (item.dataset.search || item.textContent || '').toLowerCase();
    item.classList.toggle('hidden', q.length > 0 && !haystack.includes(q));
  }
}

function pushHistory(kind, label, body, details = {}) {
  const item = document.createElement('div');
  item.className = 'history-item';
  item.dataset.kind = kind;

  const meta = document.createElement('div');
  meta.className = 'meta';
  const badge = document.createElement('span');
  badge.className = 'status-badge';
  badge.textContent = (details.status || kind || 'event').toString();
  const metaText = document.createElement('span');
  metaText.textContent = `${label} • ${nowLabel()}`;
  meta.append(badge, metaText);

  const text = document.createElement('div');
  text.className = 'text';
  text.textContent = body;

  item.append(meta, text);
  const detailBits = [];
  if (details.executionId) detailBits.push(`execution: ${details.executionId}`);
  if (details.sessionId) detailBits.push(`session: ${details.sessionId}`);
  if (Array.isArray(details.proposed) && details.proposed.length) {
    detailBits.push(`proposed: ${details.proposed.join(', ')}`);
  }
  if (Array.isArray(details.executed) && details.executed.length) {
    detailBits.push(`executed: ${details.executed.join(', ')}`);
  }
  if (Array.isArray(details.risks) && details.risks.length) {
    detailBits.push(`risk: ${details.risks.join(', ')}`);
  }
  if (detailBits.length) {
    const detailsEl = document.createElement('details');
    const summary = document.createElement('summary');
    summary.textContent = 'details';
    const pre = document.createElement('pre');
    pre.className = 'message';
    pre.textContent = detailBits.join('\n');
    detailsEl.append(summary, pre);
    item.append(detailsEl);
  }
  if (details.prompt) {
    const prompt = document.createElement('div');
    prompt.className = 'prompt';
    prompt.textContent = `you> ${details.prompt}`;
    item.append(prompt);
  }
  item.dataset.search = [label, body, details.prompt, details.executionId, details.sessionId]
    .filter(Boolean)
    .join(' ');
  activityHistoryEl.prepend(item);
  applyHistoryFilter();
}

function setActions(items) {
  const lines = Array.isArray(items) && items.length > 0 ? items : [];
  actionsEl.textContent = lines.join('\n') || '(none)';
  actionChipsEl.innerHTML = '';

  if (!lines.length) {
    actionChipsEl.innerHTML = '<span class="chip">(none)</span>';
    return;
  }

  for (const line of lines) {
    const chip = document.createElement('span');
    chip.className = 'chip';
    chip.textContent = line.length > 64 ? `${line.slice(0, 61)}...` : line;
    actionChipsEl.appendChild(chip);
  }
}

function inferRiskTier(toolName) {
  if (toolName.startsWith('time.') || toolName === 'echo') return 'ReadOnly';
  if (toolName.startsWith('desktop.') || toolName.startsWith('android.') || toolName.startsWith('ios.')) {
    return 'LocalActions';
  }
  return 'SystemActions';
}

function riskChipClass(tier) {
  return `risk-${String(tier).toLowerCase()}`;
}

function normalizeTier(tier) {
  return typeof tier === 'string' && tier ? tier : 'SystemActions';
}

function legacyEntryToActionEvent(entry) {
  const text = String(entry || '');
  if (text.startsWith('confirm_required:')) {
    const [, toolName, ...reasonParts] = text.split(':');
    const tool = toolName || '(unknown)';
    return {
      tool_name: tool,
      capability_tier: inferRiskTier(tool),
      status: 'consent_required',
      reason: reasonParts.join(':') || 'Approval required',
      arguments_preview: null,
      evidence_summary: null,
    };
  }
  if (text.startsWith('denied:')) {
    const [, toolName, ...reasonParts] = text.split(':');
    const tool = toolName || '(unknown)';
    return {
      tool_name: tool,
      capability_tier: inferRiskTier(tool),
      status: 'denied',
      reason: reasonParts.join(':') || 'Denied',
      arguments_preview: null,
      evidence_summary: null,
    };
  }
  return {
    tool_name: text,
    capability_tier: inferRiskTier(text),
    status: 'executed',
    reason: null,
    arguments_preview: null,
    evidence_summary: null,
  };
}

function normalizeActionEvents(result) {
  const events = Array.isArray(result && result.action_events) ? result.action_events : [];
  if (events.length) {
    return events.map((evt) => ({
      tool_name: evt.tool_name || '(unknown)',
      capability_tier: normalizeTier(evt.capability_tier),
      status: evt.status || 'unknown',
      reason: evt.reason || null,
      arguments_preview: evt.arguments_preview || null,
      evidence_summary: evt.evidence_summary || null,
    }));
  }
  const legacy = Array.isArray(result && result.actions_executed) ? result.actions_executed : [];
  return legacy.map(legacyEntryToActionEvent);
}

function normalizeProposedActions(result) {
  const proposed = Array.isArray(result && result.proposed_actions) ? result.proposed_actions : [];
  if (proposed.length) {
    return proposed.map((evt) => ({
      tool_name: evt.tool_name || '(unknown)',
      capability_tier: normalizeTier(evt.capability_tier),
      status: evt.status || 'unknown',
      reason: evt.reason || null,
      arguments_preview: evt.arguments_preview || null,
      evidence_summary: null,
    }));
  }

  return normalizeActionEvents(result).filter(
    (evt) => evt.status === 'consent_required' || evt.status === 'denied' || evt.status === 'approved',
  );
}

function normalizeExecutedActionEvents(result) {
  const executed = Array.isArray(result && result.executed_action_events)
    ? result.executed_action_events
    : [];
  if (executed.length) {
    return executed.map((evt) => ({
      tool_name: evt.tool_name || '(unknown)',
      capability_tier: normalizeTier(evt.capability_tier),
      status: evt.status || 'executed',
      reason: evt.reason || null,
      arguments_preview: evt.arguments_preview || null,
      evidence_summary: evt.evidence_summary || null,
    }));
  }
  return normalizeActionEvents(result).filter((evt) => evt.status === 'executed');
}

function clearConsent() {
  pendingConsent = null;
  pendingConsentFingerprint = null;
  pendingConsentToken = null;
  pendingConsentMeta = null;
  consentApprovalArmed = false;
  consentCardEl.classList.add('hidden');
  consentRequestedEl.innerHTML = '';
  consentDetailsEl.innerHTML = '';
  consentSummaryEl.textContent = 'The assistant requested an action that needs approval.';
  consentScopeEl.textContent = 'Approval scope: once, for this exact request only.';
  approveConsentBtn.textContent = 'Approve Once';
  approveConsentBtn.classList.remove('secondary');
}

function showConsent(requests, requestFingerprint, consentToken, consentRequestMeta) {
  pendingConsent = requests;
  pendingConsentFingerprint = requestFingerprint || null;
  pendingConsentToken = consentToken || null;
  pendingConsentMeta = consentRequestMeta || null;
  consentApprovalArmed = false;
  consentCardEl.classList.remove('hidden');
  consentSummaryEl.textContent =
    (consentRequestMeta && consentRequestMeta.human_summary)
      || (requests.length === 1
        ? `Approve execution of ${requests[0].toolName}?`
        : `Approve execution of ${requests.length} requested actions?`);

  consentRequestedEl.innerHTML = '';
  consentDetailsEl.innerHTML = '';
  approveConsentBtn.textContent = requiresExtraConsentClick(requests, consentRequestMeta)
    ? 'Review Risk, Click Again to Approve'
    : 'Approve Once';
  approveConsentBtn.classList.toggle('secondary', requiresExtraConsentClick(requests, consentRequestMeta));
  const scopeLabel =
    consentRequestMeta && consentRequestMeta.scope
      ? String(consentRequestMeta.scope).replaceAll('_', ' ')
      : 'once, for this exact request only';
  const extraClick = requiresExtraConsentClick(requests, consentRequestMeta);
  const riskFactors = Array.isArray(consentRequestMeta && consentRequestMeta.risk_factors)
    ? consentRequestMeta.risk_factors
    : [];
  const expiryBits = [];
  if (consentRequestMeta && consentRequestMeta.ttl_seconds) {
    expiryBits.push(`TTL: ${consentRequestMeta.ttl_seconds}s`);
  }
  if (consentRequestMeta && consentRequestMeta.expires_at_unix_seconds) {
    expiryBits.push(`expires at ${consentRequestMeta.expires_at_unix_seconds}`);
  }
  const expirySuffix = expiryBits.length ? ` ${expiryBits.join(' · ')}` : '';
  consentScopeEl.textContent = extraClick
    ? `Approval scope: ${scopeLabel} (${requestFingerprint || 'unknown'}). High-risk actions require a second confirmation click.${riskFactors.length ? ` Risks: ${riskFactors.join(', ')}` : ''}${expirySuffix}`
    : `Approval scope: ${scopeLabel} (${requestFingerprint || 'unknown'}).${riskFactors.length ? ` Risks: ${riskFactors.join(', ')}` : ''}${expirySuffix}`;

  for (const req of requests) {
    const nameChip = document.createElement('span');
    nameChip.className = 'chip';
    nameChip.textContent = req.toolName;
    consentRequestedEl.appendChild(nameChip);

    const riskChip = document.createElement('span');
    riskChip.className = `chip ${riskChipClass(req.riskTier)}`;
    riskChip.textContent = req.riskTier;
    consentRequestedEl.appendChild(riskChip);

    const detail = document.createElement('div');
    detail.className = 'event consent';
    detail.innerHTML = `
      <div class="label">${escapeHtml(req.toolName)}</div>
      <div class="body">${escapeHtml(req.reason)}${req.argumentsPreview ? `\nArgs: ${escapeHtml(req.argumentsPreview)}` : ''}</div>
    `;
    consentDetailsEl.appendChild(detail);
  }
}

function resetPanelsForRequest() {
  auditEl.textContent = 'audit_id: n/a';
  setActions([]);
  clearConsent();
}

function parsePendingConsent(actions) {
  return (Array.isArray(actions) ? actions : [])
    .filter((evt) => evt.status === 'consent_required')
    .map((evt) => ({
      raw: evt,
      toolName: evt.tool_name || '(unknown)',
      reason: evt.reason || 'Approval required',
      riskTier: normalizeTier(evt.capability_tier),
      argumentsPreview: evt.arguments_preview || null,
    }));
}

function renderChatResult(result) {
  const requestFingerprint = result.request_fingerprint || 'unknown';
  const consentToken = result.consent_token || null;
  const consentRequestMeta =
    result && result.consent_request && typeof result.consent_request === 'object'
      ? result.consent_request
      : null;
  const proposedActions = normalizeProposedActions(result);
  const executedActionEvents = normalizeExecutedActionEvents(result);
  const actionEvents = normalizeActionEvents(result);
  auditEl.textContent = `audit_id: ${result.audit_id || 'n/a'}`;
  setActions(
    Array.isArray(result.actions_executed) && result.actions_executed.length
      ? result.actions_executed
      : actionEvents.map((evt) => `${evt.status}:${evt.tool_name}`),
  );

  const pending = parsePendingConsent(proposedActions);
  if (pending.length > 0) {
    showConsent(pending, requestFingerprint, consentToken, consentRequestMeta);
    setCurrentAction(
      'consent',
      'Consent Required',
      pending
        .map((p) => `${p.toolName}: ${p.reason}${p.argumentsPreview ? `\nArgs: ${p.argumentsPreview}` : ''}`)
        .join('\n\n'),
      ['consent', requestFingerprint, ...pending.map((p) => p.riskTier)],
    );
    pushHistory('consent', 'Execution Pending Approval', pending.map((p) => `${p.toolName} (${p.riskTier})`).join(', '), {
      status: 'waiting',
      prompt: lastChatContext && lastChatContext.prompt ? lastChatContext.prompt : null,
      executionId: result.audit_id || null,
      sessionId: result.session_id || null,
      proposed: pending.map((p) => `${p.toolName}:${p.reason}`),
      risks: pending.map((p) => p.riskTier),
    });
    return;
  }

  const denied = proposedActions.filter((evt) => evt.status === 'denied');
  const executed = executedActionEvents;
  if (executed.length > 0) {
    setCurrentAction(
      'ok',
      'Action Executed',
      executed
        .map((evt) =>
          evt.evidence_summary ? `${evt.tool_name}\n${evt.evidence_summary}` : evt.tool_name,
        )
        .join('\n\n'),
      executed.map((evt) => normalizeTier(evt.capability_tier)),
    );
    pushHistory(
      'ok',
      'Execution Completed',
      (result.final_text || 'Completed').toString(),
      {
        status: 'success',
        prompt: lastChatContext && lastChatContext.prompt ? lastChatContext.prompt : null,
        executionId: result.audit_id || null,
        sessionId: result.session_id || null,
        proposed: proposedActions.map((evt) => `${evt.tool_name}:${evt.status}`),
        executed: executed.map((evt) => `${evt.tool_name}:${evt.status}`),
      },
    );
  } else if (denied.length > 0) {
    setCurrentAction(
      'warn',
      'Action Denied',
      denied.map((evt) => `${evt.tool_name}: ${evt.reason || 'Denied'}`).join('\n'),
      denied.map((evt) => normalizeTier(evt.capability_tier)),
    );
    pushHistory(
      'warn',
      'Execution Denied',
      denied.map((evt) => `${evt.tool_name}: ${evt.reason || 'Denied'}`).join(', '),
      {
        status: 'failed',
        prompt: lastChatContext && lastChatContext.prompt ? lastChatContext.prompt : null,
        executionId: result.audit_id || null,
        sessionId: result.session_id || null,
        proposed: denied.map((evt) => `${evt.tool_name}:${evt.status}`),
      },
    );
  } else {
    setCurrentAction('event', 'No Action Taken', 'The request completed without executing a tool action.', ['idle']);
    pushHistory(
      'event',
      'Execution Completed',
      (result.final_text || 'No action taken').toString(),
      {
        status: 'completed',
        prompt: lastChatContext && lastChatContext.prompt ? lastChatContext.prompt : null,
        executionId: result.audit_id || null,
        sessionId: result.session_id || null,
      },
    );
  }
}

function renderToolsResult(result) {
  if (!Array.isArray(result)) {
    setCurrentAction('warn', 'Unexpected Result', 'Unexpected tools.list result shape.', ['error']);
    pushHistory('warn', 'Unexpected Result', 'Unexpected tools.list result shape.');
    return;
  }

  auditEl.textContent = 'audit_id: n/a (tools.list)';
  setActions(result.map((t) => `${t.name} - ${t.description}`));
  const names = result.map((t) => t.name);
  setCurrentAction('event', `Tools Available (${result.length})`, names.join('\n'), ['registry']);
  pushHistory('event', `Tools Available (${result.length})`, names.join(', '));
  setWorkspaceState({ tools: result });
}

function renderJsonRpcResponse(payload) {
  setRaw(payload);

  if (payload && payload.error) {
    const body = `${payload.error.code}: ${payload.error.message}`;
    const consentError = humanizeConsentRpcError(payload.error.message || '');
    const title = consentError ? 'Consent Error' : 'JSON-RPC Error';
    setCurrentAction('warn', title, consentError || body, ['error']);
    pushHistory('warn', title, consentError || body);
    setStatus('Request failed');
    return;
  }

  const result = payload ? payload.result : null;
  if (result && typeof result === 'object' && Object.prototype.hasOwnProperty.call(result, 'final_text')) {
    renderChatResult(result);
    setStatus('Action state updated');
    return;
  }

  renderToolsResult(result);
  setStatus('Tool list received');
}

function humanizeConsentRpcError(message) {
  const msg = String(message || '');
  if (msg.includes('consent_expired') || msg.includes('consent_not_pending:expired')) {
    return 'This approval request has expired. Refresh the approvals queue and request the action again.';
  }
  if (msg.includes('consent_not_pending:approved')) {
    return 'This approval request was already approved.';
  }
  if (msg.includes('consent_not_pending:denied')) {
    return 'This approval request was already denied.';
  }
  if (msg.includes('consent_not_found')) {
    return 'This approval request no longer exists (already handled or expired).';
  }
  return null;
}

async function callJsonRpc(method, params) {
  const payload = {
    jsonrpc: '2.0',
    id: Date.now(),
    method,
    params,
  };

  setRaw(payload);
  return await transport.callJsonRpc(payload);
}

async function runChatRequest(modeOverride) {
  resetPanelsForRequest();
  const prompt = (promptEl.value || '').trim();
  setStatus('Sending chat.request...');
  setCurrentAction('event', 'Processing', prompt || '(empty prompt)', ['pending']);

  lastChatContext = {
    prompt,
    providerConfig: { provider_name: 'openai-stub', model: null },
  };
  const sessionId = sessionIdEl && typeof sessionIdEl.value === 'string' ? sessionIdEl.value.trim() : '';

  const json = await callJsonRpc('chat.request', {
    session_id: sessionId || null,
    messages: [{ role: 'user', content: prompt }],
    provider_config: lastChatContext.providerConfig,
    mode: modeOverride || (requireConfirmationEl.checked ? 'RequireConfirmation' : 'BestEffort'),
  });
  renderJsonRpcResponse(json);
}

async function runToolsList() {
  resetPanelsForRequest();
  setStatus('Requesting tools.list...');
  setCurrentAction('event', 'Loading Tool Registry', 'Fetching available tools...', ['registry']);
  const json = await callJsonRpc('tools.list', {});
  renderJsonRpcResponse(json);
}

async function runSessionsList() {
  const json = await callJsonRpc('sessions.list', {});
  setRaw(json);
  if (json && json.result) setWorkspaceState({ sessions: json.result });
  const sessions = Array.isArray(json && json.result) ? json.result : [];
  setCurrentAction('event', `Sessions (${sessions.length})`, sessions.map((s) => s.id).join('\n') || '(none)', ['sessions']);
  setStatus('Session list received');
}

async function runNewSession() {
  const json = await callJsonRpc('sessions.create', { title: null });
  setRaw(json);
  const result = json && json.result ? json.result : null;
  if (result && result.id && sessionIdEl) {
    sessionIdEl.value = String(result.id);
  }
  setWorkspaceState({ newSession: result });
  setCurrentAction('event', 'Session Created', result && result.id ? String(result.id) : 'unknown', ['sessions']);
  setStatus('Session created');
}

async function runProvidersList() {
  const json = await callJsonRpc('providers.list', {});
  setRaw(json);
  if (json && json.result) setWorkspaceState({ providers: json.result });
  const providers = Array.isArray(json && json.result) ? json.result : [];
  setCurrentAction('event', `Providers (${providers.length})`, providers.map((p) => `${p.name} ${p.is_active ? '(active)' : ''}`).join('\n'), ['providers']);
  setStatus('Provider list received');
}

async function runConsentQueue() {
  const sessionId = sessionIdEl && typeof sessionIdEl.value === 'string' ? sessionIdEl.value.trim() : '';
  const json = await callJsonRpc('consent.list', { status: 'pending', session_id: sessionId || null });
  setRaw(json);
  if (json && json.result) setWorkspaceState({ approvals: json.result });
  const consents = Array.isArray(json && json.result) ? json.result : [];
  setCurrentAction('event', `Approvals Queue (${consents.length})`, consents.map((c) => `${c.consent_id}: ${c.tool_name}`).join('\n') || '(empty)', ['consent-queue']);
  setStatus('Approvals queue received');
}

async function runAuditList() {
  const sessionId = sessionIdEl && typeof sessionIdEl.value === 'string' ? sessionIdEl.value.trim() : '';
  const json = await callJsonRpc('audit.list', { session_id: sessionId || null, limit: 20 });
  setRaw(json);
  if (json && json.result) setWorkspaceState({ audit: json.result });
  const audits = Array.isArray(json && json.result) ? json.result : [];
  setCurrentAction('event', `Audit (${audits.length})`, audits.map((a) => `${a.audit_id} ${a.provider}`).join('\n') || '(empty)', ['audit']);
  setStatus('Audit list received');
}

async function runProjectOpen() {
  const path =
    projectPathEl && typeof projectPathEl.value === 'string' && projectPathEl.value.trim()
      ? projectPathEl.value.trim()
      : '.';
  const json = await callJsonRpc('project.open', { path });
  setRaw(json);
  if (json && json.result) {
    setWorkspaceState({ projectOpen: json.result });
    if (json.result.path && projectPathEl) projectPathEl.value = String(json.result.path);
    setCurrentAction(
      json.result.exists && json.result.is_dir ? 'ok' : 'warn',
      'Project Open',
      `${json.result.path}\nexists=${json.result.exists}\nis_dir=${json.result.is_dir}`,
      ['project'],
    );
    pushHistory('event', 'Project Open', String(json.result.path || path));
    setStatus(json.result.exists && json.result.is_dir ? 'Project opened' : 'Project path invalid');
    return;
  }
  setStatus('Project open request completed');
}

async function runProjectStatus() {
  const path =
    projectPathEl && typeof projectPathEl.value === 'string' && projectPathEl.value.trim()
      ? projectPathEl.value.trim()
      : null;
  const json = await callJsonRpc('project.status', { path });
  setRaw(json);
  if (json && json.result) {
    setWorkspaceState({ projectStatus: json.result });
    if (json.result.path && projectPathEl) projectPathEl.value = String(json.result.path);
    setCurrentAction(
      'event',
      'Project Status',
      `${json.result.path}\nexists=${json.result.exists}\nis_dir=${json.result.is_dir}\nentries=${json.result.entry_count}`,
      ['project'],
    );
    pushHistory('event', 'Project Status', String(json.result.path || '(unknown)'));
    setStatus('Project status received');
    return;
  }
  setStatus('Project status request completed');
}

async function approvePendingConsent() {
  if (!pendingConsent || (!pendingConsentToken && !lastChatContext)) {
    setStatus('No pending consent request');
    return;
  }

  if (requiresExtraConsentClick(pendingConsent, pendingConsentMeta) && !consentApprovalArmed) {
    consentApprovalArmed = true;
    approveConsentBtn.textContent = 'Confirm Risky Action';
    approveConsentBtn.classList.remove('secondary');
    setStatus('High-risk action detected. Click approve again to confirm.');
    setCurrentAction(
      'consent',
      'Confirm Risky Action',
      pendingConsent.map((p) => `${p.toolName} (${p.riskTier})`).join('\n'),
      ['consent', 'high-risk', pendingConsentFingerprint || 'unknown'],
    );
    return;
  }

  setCurrentAction(
    'ok',
    'User Approved',
    pendingConsent.map((p) => `${p.toolName} (${p.riskTier})`).join('\n'),
    ['consent', 'approved', pendingConsentFingerprint || 'unknown'],
  );
  pushHistory('ok', 'User Approved', pendingConsent.map((p) => p.toolName).join(', '));
  clearConsent();
  setStatus(
    pendingConsentToken
      ? 'Sending chat.approve...'
      : 'Re-running request with one-time approval...',
  );

  const json = pendingConsentToken
    ? await callJsonRpc('chat.approve', { consent_token: pendingConsentToken })
    : await callJsonRpc('chat.request', {
        messages: [{ role: 'user', content: lastChatContext.prompt }],
        provider_config: lastChatContext.providerConfig,
        mode: 'BestEffort',
      });
  renderJsonRpcResponse(json);
}

async function withUiBusy(action) {
  sendBtn.disabled = true;
  listToolsBtn.disabled = true;
  clearViewBtn.disabled = true;
  if (newSessionBtn) newSessionBtn.disabled = true;
  if (listSessionsBtn) listSessionsBtn.disabled = true;
  if (listProvidersBtn) listProvidersBtn.disabled = true;
  if (listConsentsBtn) listConsentsBtn.disabled = true;
  if (listAuditBtn) listAuditBtn.disabled = true;
  if (openProjectBtn) openProjectBtn.disabled = true;
  if (projectStatusBtn) projectStatusBtn.disabled = true;
  approveConsentBtn.disabled = true;
  denyConsentBtn.disabled = true;

  try {
    await action();
  } catch (error) {
    const message =
      'Backend unavailable. Start `bash scripts/dev-prototype.sh` (dev) or wire Tauri backend transport.';
    setCurrentAction('warn', 'Connection Error', message, ['offline']);
    pushHistory('warn', 'Connection Error', message);
    setStatus(error instanceof Error ? `Connection failed: ${error.message}` : 'Connection failed');
  } finally {
    sendBtn.disabled = false;
    listToolsBtn.disabled = false;
    clearViewBtn.disabled = false;
    if (newSessionBtn) newSessionBtn.disabled = false;
    if (listSessionsBtn) listSessionsBtn.disabled = false;
    if (listProvidersBtn) listProvidersBtn.disabled = false;
    if (listConsentsBtn) listConsentsBtn.disabled = false;
    if (listAuditBtn) listAuditBtn.disabled = false;
    if (openProjectBtn) openProjectBtn.disabled = false;
    if (projectStatusBtn) projectStatusBtn.disabled = false;
    approveConsentBtn.disabled = false;
    denyConsentBtn.disabled = false;
  }
}

function requiresExtraConsentClick(requests, consentMeta) {
  if (consentMeta && typeof consentMeta.requires_extra_confirmation_click === 'boolean') {
    return consentMeta.requires_extra_confirmation_click;
  }
  return (Array.isArray(requests) ? requests : []).some(
    (req) => req.riskTier === 'LocalActions' || req.riskTier === 'SystemActions',
  );
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function getTauriInvoke() {
  const tauri = window.__TAURI__;
  if (tauri && tauri.core && typeof tauri.core.invoke === 'function') {
    return tauri.core.invoke.bind(tauri.core);
  }
  const internals = window.__TAURI_INTERNALS__;
  if (internals && typeof internals.invoke === 'function') {
    return internals.invoke.bind(internals);
  }
  return null;
}

function detectTransport() {
  const tauriInvoke = getTauriInvoke();
  if (tauriInvoke) {
    transportBadgeEl.textContent = 'transport: tauri-bridge';
    return {
      name: 'tauri-bridge',
      async callJsonRpc(payload) {
        const result = await tauriInvoke('jsonrpc_request', {
          payloadJson: JSON.stringify(payload),
        });
        return typeof result === 'string' ? JSON.parse(result) : result;
      },
    };
  }

  transportBadgeEl.textContent = 'transport: http-dev';
  return {
    name: 'http-dev',
    async callJsonRpc(payload) {
      const resp = await fetch(JSONRPC_URL, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (!resp.ok) {
        throw new Error(`HTTP ${resp.status}: ${await resp.text()}`);
      }
      return await resp.json();
    },
  };
}

sendBtn.addEventListener('click', async () => {
  await withUiBusy(async () => {
    await runChatRequest();
  });
});

listToolsBtn.addEventListener('click', async () => {
  await withUiBusy(runToolsList);
});

clearViewBtn.addEventListener('click', () => {
  auditEl.textContent = 'audit_id: n/a';
  setActions([]);
  clearConsent();
  clearHistory();
  setCurrentAction('event', 'Ready', 'No actions yet.', ['idle']);
  setRaw('No requests yet.');
  setWorkspaceState('(no workspace data loaded)');
  setStatus('Cleared');
});

if (newSessionBtn) {
  newSessionBtn.addEventListener('click', async () => {
    await withUiBusy(runNewSession);
  });
}
if (listSessionsBtn) {
  listSessionsBtn.addEventListener('click', async () => {
    await withUiBusy(runSessionsList);
  });
}
if (listProvidersBtn) {
  listProvidersBtn.addEventListener('click', async () => {
    await withUiBusy(runProvidersList);
  });
}
if (openProjectBtn) {
  openProjectBtn.addEventListener('click', async () => {
    await withUiBusy(runProjectOpen);
  });
}
if (projectStatusBtn) {
  projectStatusBtn.addEventListener('click', async () => {
    await withUiBusy(runProjectStatus);
  });
}
if (listConsentsBtn) {
  listConsentsBtn.addEventListener('click', async () => {
    await withUiBusy(runConsentQueue);
  });
}
if (listAuditBtn) {
  listAuditBtn.addEventListener('click', async () => {
    await withUiBusy(runAuditList);
  });
}

approveConsentBtn.addEventListener('click', async () => {
  await withUiBusy(approvePendingConsent);
});

denyConsentBtn.addEventListener('click', async () => {
  if (pendingConsent) {
    setCurrentAction(
      'warn',
      'User Denied',
      pendingConsent.map((p) => `${p.toolName} (${p.riskTier})`).join('\n'),
      ['consent', 'denied'],
    );
    pushHistory('warn', 'User Denied', pendingConsent.map((p) => p.toolName).join(', '));
  }
  const consentToken = pendingConsentToken;
  clearConsent();

  if (consentToken) {
    await withUiBusy(async () => {
      setStatus('Sending chat.deny...');
      const json = await callJsonRpc('chat.deny', { consent_token: consentToken });
      renderJsonRpcResponse(json);
    });
    return;
  }

  setStatus('Consent denied');
});

presetButtons.forEach((button) => {
  button.addEventListener('click', () => {
    promptEl.value = button.dataset.prompt || '';
    promptEl.focus();
    setStatus('Preset loaded');
  });
});

debugToggleBtn.addEventListener('click', () => {
  const isOpen = shellEl.classList.toggle('debug-open');
  debugToggleBtn.setAttribute('aria-pressed', String(isOpen));
  debugToggleBtn.textContent = isOpen ? 'Debug On' : 'Debug';
});

promptEl.addEventListener('keydown', async (event) => {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    await withUiBusy(async () => {
      await runChatRequest();
    });
  }
});

if (historySearchEl) {
  historySearchEl.addEventListener('input', () => {
    historyFilter = historySearchEl.value || '';
    applyHistoryFilter();
  });
}

transport = detectTransport();
clearHistory();
clearConsent();
setCurrentAction('event', 'Ready', 'No actions yet.', ['idle']);
setStatus(`Ready (${transport.name})`);
