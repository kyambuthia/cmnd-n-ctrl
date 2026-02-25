const promptEl = document.querySelector('#prompt');
const activityFeedEl = document.querySelector('#activityFeed');
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
const approveConsentBtn = document.querySelector('#approveConsent');
const denyConsentBtn = document.querySelector('#denyConsent');
const presetButtons = Array.from(document.querySelectorAll('.prompt-preset'));

const JSONRPC_URL = 'http://127.0.0.1:7777/jsonrpc';
let transport = null;
let lastChatContext = null;
let pendingConsent = null;

function setStatus(message) {
  statusEl.innerHTML = `<span class="dot"></span>${escapeHtml(message)}`;
}

function setRaw(payload) {
  rawEl.textContent = typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2);
}

function clearActivity() {
  activityFeedEl.innerHTML = `
    <div class="event">
      <div class="label">Ready</div>
      <div class="body">No actions yet.</div>
    </div>
  `;
}

function appendActivity(kind, label, body) {
  const item = document.createElement('div');
  item.className = `event ${kind}`;

  const labelEl = document.createElement('div');
  labelEl.className = 'label';
  labelEl.textContent = label;

  const bodyEl = document.createElement('div');
  bodyEl.className = 'body';
  bodyEl.textContent = body;

  item.append(labelEl, bodyEl);
  activityFeedEl.prepend(item);
}

function setActions(items) {
  const lines = Array.isArray(items) && items.length > 0 ? items : [];
  actionsEl.textContent = lines.join('\n') || '(none)';
  actionChipsEl.innerHTML = '';

  if (lines.length === 0) {
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

function clearConsent() {
  pendingConsent = null;
  consentCardEl.classList.add('hidden');
  consentRequestedEl.innerHTML = '';
  consentSummaryEl.textContent = 'The assistant requested an action that needs approval.';
}

function showConsent(requests) {
  pendingConsent = requests;
  consentCardEl.classList.remove('hidden');
  consentSummaryEl.textContent =
    requests.length === 1
      ? `Approve execution of ${requests[0].toolName}?`
      : `Approve execution of ${requests.length} requested actions?`;
  consentRequestedEl.innerHTML = '';

  for (const req of requests) {
    const chip = document.createElement('span');
    chip.className = 'chip';
    chip.title = req.reason;
    chip.textContent = req.toolName;
    consentRequestedEl.appendChild(chip);
  }
}

function resetPanelsForRequest() {
  auditEl.textContent = 'audit_id: n/a';
  setActions([]);
  clearConsent();
}

function parsePendingConsent(actions) {
  return (Array.isArray(actions) ? actions : [])
    .filter((a) => typeof a === 'string' && a.startsWith('confirm_required:'))
    .map((entry) => {
      const [, toolName, ...reasonParts] = entry.split(':');
      return {
        raw: entry,
        toolName: toolName || '(unknown)',
        reason: reasonParts.join(':') || 'Approval required',
      };
    });
}

function renderChatResult(result) {
  const actions = result.actions_executed || [];
  auditEl.textContent = `audit_id: ${result.audit_id || 'n/a'}`;
  setActions(actions);

  const pending = parsePendingConsent(actions);
  if (pending.length > 0) {
    showConsent(pending);
    appendActivity('consent', 'Consent Needed', pending.map((p) => `${p.toolName}: ${p.reason}`).join('\n'));
    return;
  }

  const executed = actions.filter((a) => !String(a).startsWith('confirm_required:'));
  if (executed.length > 0) {
    appendActivity('ok', 'Action Executed', executed.join('\n'));
  } else if (result.final_text) {
    appendActivity('event', 'No Action Taken', 'The request completed without executing a tool action.');
  } else {
    appendActivity('event', 'No Action Taken', '(no action reported)');
  }
}

function renderToolsResult(result) {
  if (!Array.isArray(result)) {
    appendActivity('warn', 'Unexpected Result', 'Unexpected tools.list result shape.');
    return;
  }

  auditEl.textContent = 'audit_id: n/a (tools.list)';
  setActions(result.map((t) => `${t.name} - ${t.description}`));
  appendActivity('event', `Tools Available (${result.length})`, result.map((t) => t.name).join('\n'));
}

function renderJsonRpcResponse(payload) {
  setRaw(payload);

  if (payload && payload.error) {
    appendActivity('warn', 'JSON-RPC Error', `${payload.error.code}: ${payload.error.message}`);
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
  const json = await callJsonRpc('tools.list', {});
  renderJsonRpcResponse(json);
}

async function approvePendingConsent() {
  if (!pendingConsent || !lastChatContext) {
    setStatus('No pending consent request');
    return;
  }

  appendActivity('ok', 'User Approved', pendingConsent.map((p) => p.toolName).join('\n'));
  clearConsent();
  setStatus('Re-running request with one-time approval...');

  const json = await callJsonRpc('chat.request', {
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
    appendActivity(
      'warn',
      'Connection Error',
      'Backend unavailable. Start `bash scripts/dev-prototype.sh` (dev) or wire Tauri backend transport.',
    );
    setStatus(error instanceof Error ? `Connection failed: ${error.message}` : 'Connection failed');
  } finally {
    sendBtn.disabled = false;
    listToolsBtn.disabled = false;
    clearViewBtn.disabled = false;
    approveConsentBtn.disabled = false;
    denyConsentBtn.disabled = false;
  }
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
        if (typeof result === 'string') {
          return JSON.parse(result);
        }
        return result;
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
  clearActivity();
  setRaw('No requests yet.');
  setStatus('Cleared');
});

approveConsentBtn.addEventListener('click', async () => {
  await withUiBusy(approvePendingConsent);
});

denyConsentBtn.addEventListener('click', () => {
  if (pendingConsent) {
    appendActivity('warn', 'User Denied', pendingConsent.map((p) => p.toolName).join('\n'));
  }
  clearConsent();
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
clearActivity();
clearConsent();
setStatus(`Ready (${transport.name})`);
