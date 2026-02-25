const promptEl = document.querySelector('#prompt');
const responseEl = document.querySelector('#response');
const auditEl = document.querySelector('#audit');
const actionsEl = document.querySelector('#actions');
const actionChipsEl = document.querySelector('#actionChips');
const rawEl = document.querySelector('#raw');
const statusEl = document.querySelector('#status');
const transportBadgeEl = document.querySelector('#transportBadge');
const requireConfirmationEl = document.querySelector('#requireConfirmation');
const sendBtn = document.querySelector('#send');
const listToolsBtn = document.querySelector('#listTools');
const clearViewBtn = document.querySelector('#clearView');
const presetButtons = Array.from(document.querySelectorAll('.prompt-preset'));

const JSONRPC_URL = 'http://127.0.0.1:7777/jsonrpc';
let transport = null;

function setStatus(message) {
  statusEl.innerHTML = `<span class="dot"></span>${escapeHtml(message)}`;
}

function setRaw(payload) {
  rawEl.textContent = typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2);
}

function resetPanelsForRequest() {
  auditEl.textContent = 'audit_id: n/a';
  setActions([]);
}

function renderChatResult(result) {
  responseEl.textContent = result.final_text || '';
  auditEl.textContent = `audit_id: ${result.audit_id || 'n/a'}`;
  setActions(result.actions_executed || []);
}

function renderToolsResult(result) {
  if (!Array.isArray(result)) {
    responseEl.textContent = 'Unexpected tools.list result shape.';
    return;
  }

  responseEl.textContent = `Available tools: ${result.length}`;
  auditEl.textContent = 'audit_id: n/a (tools.list)';
  setActions(result.map((t) => `${t.name} - ${t.description}`));
}

function renderJsonRpcResponse(payload) {
  setRaw(payload);

  if (payload && payload.error) {
    responseEl.textContent = `JSON-RPC error ${payload.error.code}: ${payload.error.message}`;
    setStatus('Request failed');
    return;
  }

  const result = payload ? payload.result : null;
  if (result && typeof result === 'object' && Object.prototype.hasOwnProperty.call(result, 'final_text')) {
    renderChatResult(result);
    setStatus('Chat response received');
    return;
  }

  renderToolsResult(result);
  setStatus('Tool list received');
}

async function callLocalJsonRpc(method, params) {
  const payload = {
    jsonrpc: '2.0',
    id: Date.now(),
    method,
    params,
  };

  setRaw(payload);

  return await transport.callJsonRpc(payload);
}

async function runChatRequest() {
  resetPanelsForRequest();
  setStatus('Sending chat.request...');
  responseEl.textContent = 'Thinking...';

  const json = await callLocalJsonRpc('chat.request', {
    messages: [{ role: 'user', content: promptEl.value }],
    provider_config: { provider_name: 'openai-stub', model: null },
    mode: requireConfirmationEl.checked ? 'RequireConfirmation' : 'BestEffort',
  });
  renderJsonRpcResponse(json);
}

async function runToolsList() {
  resetPanelsForRequest();
  setStatus('Requesting tools.list...');
  responseEl.textContent = 'Loading tools...';

  const json = await callLocalJsonRpc('tools.list', {});
  renderJsonRpcResponse(json);
}

async function withUiBusy(action) {
  sendBtn.disabled = true;
  listToolsBtn.disabled = true;
  clearViewBtn.disabled = true;
  try {
    await action();
  } catch (error) {
    responseEl.textContent =
      'Backend unavailable. Start `bash scripts/dev-prototype.sh` (dev) or wire Tauri backend transport.';
    setStatus(error instanceof Error ? `Connection failed: ${error.message}` : 'Connection failed');
  } finally {
    sendBtn.disabled = false;
    listToolsBtn.disabled = false;
    clearViewBtn.disabled = false;
  }
}

sendBtn.addEventListener('click', async () => {
  await withUiBusy(runChatRequest);
});

listToolsBtn.addEventListener('click', async () => {
  await withUiBusy(runToolsList);
});

clearViewBtn.addEventListener('click', () => {
  responseEl.textContent = 'No response yet.';
  auditEl.textContent = 'audit_id: n/a';
  setActions([]);
  setRaw('No requests yet.');
  setStatus('Cleared');
});

presetButtons.forEach((button) => {
  button.addEventListener('click', () => {
    promptEl.value = button.dataset.prompt || '';
    promptEl.focus();
    setStatus('Preset loaded');
  });
});

promptEl.addEventListener('keydown', async (event) => {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    await withUiBusy(runChatRequest);
  }
});

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

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function detectTransport() {
  const tauriInvoke = getTauriInvoke();
  if (tauriInvoke) {
    transportBadgeEl.textContent = 'transport: tauri-bridge';
    return {
      name: 'tauri-bridge',
      async callJsonRpc(payload) {
        // Expected Tauri backend command contract:
        // invoke('jsonrpc_request', { payloadJson: JSON.stringify(payload) }) -> JSON string/object
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

function isTauriRuntime() {
  return Boolean(window.__TAURI__ || window.__TAURI_INTERNALS__ || window.__TAURI_IPC__);
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

transport = detectTransport();
setStatus(`Ready (${transport.name})`);
