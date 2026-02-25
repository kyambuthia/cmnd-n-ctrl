const promptEl = document.querySelector('#prompt');
const currentActionLabelEl = document.querySelector('#currentActionLabel');
const currentActionBodyEl = document.querySelector('#currentActionBody');
const currentActionMetaEl = document.querySelector('#currentActionMeta');
const activityHistoryEl = document.querySelector('#activityHistory');
const auditEl = document.querySelector('#audit');
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

function nowLabel() {
  return new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function setStatus(message) {
  statusEl.innerHTML = `<span class="dot"></span>${escapeHtml(message)}`;
}

function setRaw(payload) {
  rawEl.textContent = typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2);
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

function pushHistory(kind, label, body) {
  const item = document.createElement('div');
  item.className = 'history-item';

  const meta = document.createElement('div');
  meta.className = 'meta';
  meta.textContent = `${label} • ${nowLabel()}`;

  const text = document.createElement('div');
  text.className = 'text';
  text.textContent = body;

  item.dataset.kind = kind;
  item.append(meta, text);
  activityHistoryEl.prepend(item);
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
  consentApprovalArmed = false;
  consentCardEl.classList.add('hidden');
  consentRequestedEl.innerHTML = '';
  consentDetailsEl.innerHTML = '';
  consentSummaryEl.textContent = 'The assistant requested an action that needs approval.';
  consentScopeEl.textContent = 'Approval scope: once, for this exact request only.';
  approveConsentBtn.textContent = 'Approve Once';
  approveConsentBtn.classList.remove('secondary');
}

function showConsent(requests, requestFingerprint, consentToken) {
  pendingConsent = requests;
  pendingConsentFingerprint = requestFingerprint || null;
  pendingConsentToken = consentToken || null;
  consentApprovalArmed = false;
  consentCardEl.classList.remove('hidden');
  consentSummaryEl.textContent =
    requests.length === 1
      ? `Approve execution of ${requests[0].toolName}?`
      : `Approve execution of ${requests.length} requested actions?`;

  consentRequestedEl.innerHTML = '';
  consentDetailsEl.innerHTML = '';
  approveConsentBtn.textContent = requiresExtraConsentClick(requests)
    ? 'Review Risk, Click Again to Approve'
    : 'Approve Once';
  approveConsentBtn.classList.toggle('secondary', requiresExtraConsentClick(requests));
  consentScopeEl.textContent = requiresExtraConsentClick(requests)
    ? `Approval scope: once, for this exact request only (${requestFingerprint || 'unknown'}). High-risk actions require a second confirmation click.`
    : `Approval scope: once, for this exact request only (${requestFingerprint || 'unknown'}).`;

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
    showConsent(pending, requestFingerprint, consentToken);
    setCurrentAction(
      'consent',
      'Consent Required',
      pending
        .map((p) => `${p.toolName}: ${p.reason}${p.argumentsPreview ? `\nArgs: ${p.argumentsPreview}` : ''}`)
        .join('\n\n'),
      ['consent', requestFingerprint, ...pending.map((p) => p.riskTier)],
    );
    pushHistory('consent', 'Consent Needed', pending.map((p) => p.toolName).join(', '));
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
    pushHistory('ok', 'Action Executed', executed.map((evt) => evt.tool_name).join(', '));
  } else if (denied.length > 0) {
    setCurrentAction(
      'warn',
      'Action Denied',
      denied.map((evt) => `${evt.tool_name}: ${evt.reason || 'Denied'}`).join('\n'),
      denied.map((evt) => normalizeTier(evt.capability_tier)),
    );
    pushHistory('warn', 'Action Denied', denied.map((evt) => evt.tool_name).join(', '));
  } else {
    setCurrentAction('event', 'No Action Taken', 'The request completed without executing a tool action.', ['idle']);
    pushHistory('event', 'No Action Taken', 'Request completed without executing a tool action.');
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
}

function renderJsonRpcResponse(payload) {
  setRaw(payload);

  if (payload && payload.error) {
    const body = `${payload.error.code}: ${payload.error.message}`;
    setCurrentAction('warn', 'JSON-RPC Error', body, ['error']);
    pushHistory('warn', 'JSON-RPC Error', body);
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

  const json = await callJsonRpc('chat.request', {
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

async function approvePendingConsent() {
  if (!pendingConsent || (!pendingConsentToken && !lastChatContext)) {
    setStatus('No pending consent request');
    return;
  }

  if (requiresExtraConsentClick(pendingConsent) && !consentApprovalArmed) {
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
    approveConsentBtn.disabled = false;
    denyConsentBtn.disabled = false;
  }
}

function requiresExtraConsentClick(requests) {
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
  setStatus('Cleared');
});

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

transport = detectTransport();
clearHistory();
clearConsent();
setCurrentAction('event', 'Ready', 'No actions yet.', ['idle']);
setStatus(`Ready (${transport.name})`);
