import argparse
import base64
import os
import sys

import pyperclip
import requests
from dotenv import load_dotenv
from mss import mss, tools


def capture_primary_screen_png_bytes() -> bytes:
    with mss() as sct:
        monitor = sct.monitors[1]
        shot = sct.grab(monitor)
        return tools.to_png(shot.rgb, shot.size)


def png_to_data_uri(png_bytes: bytes) -> str:
    b64 = base64.b64encode(png_bytes).decode("utf-8")
    return f"data:image/png;base64,{b64}"


def deepinfra_ocr(data_uri: str, token: str, model: str, base_url: str) -> str:
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
    }
    payload = {
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": (
                            "Perform OCR on this screenshot. Return only plain extracted text, "
                            "preserving reading order. Do not add commentary."
                        ),
                    },
                    {"type": "image_url", "image_url": {"url": data_uri}},
                ],
            }
        ],
        "temperature": 0,
    }

    resp = requests.post(base_url, headers=headers, json=payload, timeout=90)
    resp.raise_for_status()
    data = resp.json()

    try:
        return data["choices"][0]["message"]["content"].strip()
    except Exception:
        return str(data)


def resolve_deepinfra_token() -> str:
    return (
        os.getenv("DEEPINFRA_API_TOKEN", "").strip()
        or os.getenv("DEEPINFRA_API_KEY", "").strip()
        or os.getenv("DEEPINFRA_TOKEN", "").strip()
        or os.getenv("DEEPINFRA", "").strip()
    )


def main() -> int:
    load_dotenv()

    parser = argparse.ArgumentParser(description="Capture screen and OCR via DeepInfra")
    parser.add_argument("--save", help="Save OCR text to a file path")
    parser.add_argument("--copy", action="store_true", help="Copy OCR text to clipboard")
    args = parser.parse_args()

    token = resolve_deepinfra_token()
    model = os.getenv("DEEPINFRA_MODEL", "deepseek-ai/DeepSeek-OCR").strip()
    base_url = os.getenv(
        "DEEPINFRA_BASE_URL", "https://api.deepinfra.com/v1/openai/chat/completions"
    ).strip()

    if not token or token == "your_token_here":
        print(
            "Missing DeepInfra token. Set one of: DEEPINFRA_API_TOKEN, DEEPINFRA_API_KEY, DEEPINFRA_TOKEN, DEEPINFRA",
            file=sys.stderr,
        )
        return 1

    try:
        png_bytes = capture_primary_screen_png_bytes()
        data_uri = png_to_data_uri(png_bytes)
        text = deepinfra_ocr(data_uri, token, model, base_url)
    except requests.HTTPError as exc:
        detail = exc.response.text if exc.response is not None else str(exc)
        print(f"DeepInfra request failed: {detail}", file=sys.stderr)
        return 2
    except Exception as exc:
        print(f"Failed: {exc}", file=sys.stderr)
        return 3

    if args.save:
        with open(args.save, "w", encoding="utf-8") as f:
            f.write(text)

    if args.copy:
        pyperclip.copy(text)

    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
