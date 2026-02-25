const promptEl = document.querySelector<HTMLTextAreaElement>('#prompt')!;
const responseEl = document.querySelector<HTMLDivElement>('#response')!;
const requireConfirmationEl = document.querySelector<HTMLInputElement>('#requireConfirmation')!;
const sendBtn = document.querySelector<HTMLButtonElement>('#send')!;

type JsonRpcSuccess = {
  jsonrpc: '2.0';
  id: number | string | null;
  result?: {
    final_text?: string;
    audit_id?: string;
    actions_executed?: string[];
  } | unknown;
  error?: {
    code: number;
    message: string;
  };
};

function renderJsonRpcResponse(payload: JsonRpcSuccess): string {
  if (payload.error) {
    return `JSON-RPC error ${payload.error.code}: ${payload.error.message}`;
  }

  const result = payload.result as JsonRpcSuccess['result'] | undefined;
  if (result && typeof result === 'object' && result !== null && 'final_text' in result) {
    const typed = result as { final_text?: string; audit_id?: string; actions_executed?: string[] };
    const lines = [
      `Response: ${typed.final_text ?? ''}`,
      `Audit ID: ${typed.audit_id ?? 'n/a'}`,
      `Actions: ${(typed.actions_executed ?? []).join(', ') || '(none)'}`,
    ];
    return lines.join('\n');
  }

  return JSON.stringify(payload.result, null, 2);
}

async function callLocalJsonRpc(prompt: string, requireConfirmation: boolean): Promise<string> {
  const payload = {
    jsonrpc: '2.0',
    id: 1,
    method: 'chat.request',
    params: {
      messages: [{ role: 'user', content: prompt }],
      provider_config: { provider_name: 'openai-stub', model: null },
      mode: requireConfirmation ? 'RequireConfirmation' : 'BestEffort',
    },
  };

  try {
    // TODO: In Tauri dev, invoke backend command or connect to a local socket/child process.
    const resp = await fetch('http://127.0.0.1:7777/jsonrpc', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
    });
    if (!resp.ok) {
      return `HTTP error ${resp.status}: ${await resp.text()}`;
    }

    const json = (await resp.json()) as JsonRpcSuccess;
    return renderJsonRpcResponse(json);
  } catch {
    return 'Local JSON-RPC endpoint not running. Start `cargo run -p cli -- serve-http` and retry.';
  }
}

sendBtn.addEventListener('click', async () => {
  sendBtn.disabled = true;
  responseEl.textContent = 'Thinking...';
  responseEl.textContent = await callLocalJsonRpc(promptEl.value, requireConfirmationEl.checked);
  sendBtn.disabled = false;
});
