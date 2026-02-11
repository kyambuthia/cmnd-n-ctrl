# Screen OCR

Small local CLI tool to:
1. Capture the current screen
2. Send the image to DeepInfra `deepseek-ai/DeepSeek-OCR`
3. Print extracted text to stdout so you can pipe into Codex CLI

## Setup (Windows PowerShell)

```powershell
cd "C:\Users\classic springs\screen-ocr"
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt
```

Set your API token (recommended as an environment variable; any one of these works):

```powershell
$env:DEEPINFRA_API_TOKEN="your_real_token"\n# or: DEEPINFRA_API_KEY / DEEPINFRA_TOKEN / DEEPINFRA
```

Optional overrides:

```powershell
$env:DEEPINFRA_MODEL="deepseek-ai/DeepSeek-OCR"
$env:DEEPINFRA_BASE_URL="https://api.deepinfra.com/v1/openai/chat/completions"
```

If you prefer file-based config:

```powershell
Copy-Item .env.example .env
# then edit .env
```

## Run

```powershell
python src/capture_ocr.py --copy --save last_ocr.txt
```

## Use with Codex CLI

```powershell
python src/capture_ocr.py | codex
```

or:

```powershell
$txt = python src/capture_ocr.py
codex "$txt"
```

## Notes

- Default model: `deepseek-ai/DeepSeek-OCR`
- Env var `DEEPINFRA_MODEL` can override the default model.
- Captures the primary monitor by default.

