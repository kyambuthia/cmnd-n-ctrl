const promptEl = document.querySelector<HTMLTextAreaElement>('#prompt')!;
const responseEl = document.querySelector<HTMLDivElement>('#response')!;
const requireConfirmationEl = document.querySelector<HTMLInputElement>('#requireConfirmation')!;
const sendBtn = document.querySelector<HTMLButtonElement>('#send')!;

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
    return await resp.text();
  } catch {
    return 'Local JSON-RPC endpoint not running. TODO: wire Tauri backend to spawn/connect to core IPC.';
  }
}

sendBtn.addEventListener('click', async () => {
  sendBtn.disabled = true;
  responseEl.textContent = 'Thinking...';
  responseEl.textContent = await callLocalJsonRpc(promptEl.value, requireConfirmationEl.checked);
  sendBtn.disabled = false;
});
