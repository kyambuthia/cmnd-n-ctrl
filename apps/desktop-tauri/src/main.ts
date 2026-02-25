const promptEl = document.querySelector('#prompt');
const responseEl = document.querySelector('#response');
const auditEl = document.querySelector('#audit');
const actionsEl = document.querySelector('#actions');
const rawEl = document.querySelector('#raw');
const statusEl = document.querySelector('#status');
const requireConfirmationEl = document.querySelector('#requireConfirmation');
const sendBtn = document.querySelector('#send');
const listToolsBtn = document.querySelector('#listTools');

const JSONRPC_URL = 'http://127.0.0.1:7777/jsonrpc';

function setStatus(message) {
  statusEl.textContent = message;
}

function setRaw(payload) {
  rawEl.textContent = typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2);
}

function resetPanelsForRequest() {
  auditEl.textContent = 'audit_id: n/a';
  actionsEl.textContent = '(none)';
}

function renderChatResult(result) {
  responseEl.textContent = result.final_text || '';
  auditEl.textContent = `audit_id: ${result.audit_id || 'n/a'}`;
  actionsEl.textContent = (result.actions_executed || []).join('\n') || '(none)';
}

function renderToolsResult(result) {
  if (!Array.isArray(result)) {
    responseEl.textContent = 'Unexpected tools.list result shape.';
    return;
  }

  responseEl.textContent = `Available tools: ${result.length}`;
  auditEl.textContent = 'audit_id: n/a (tools.list)';
  actionsEl.textContent =
    result.map((t) => `${t.name} - ${t.description}`).join('\n') || '(none)';
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

  const resp = await fetch(JSONRPC_URL, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  });

  if (!resp.ok) {
    throw new Error(`HTTP ${resp.status}: ${await resp.text()}`);
  }

  return await resp.json();
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
  try {
    await action();
  } catch (error) {
    responseEl.textContent =
      'Local JSON-RPC endpoint not running. Start `cargo run -p cli -- serve-http` and retry.';
    setStatus(error instanceof Error ? `Connection failed: ${error.message}` : 'Connection failed');
  } finally {
    sendBtn.disabled = false;
    listToolsBtn.disabled = false;
  }
}

sendBtn.addEventListener('click', async () => {
  await withUiBusy(runChatRequest);
});

listToolsBtn.addEventListener('click', async () => {
  await withUiBusy(runToolsList);
});
