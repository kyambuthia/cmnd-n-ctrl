import { createServer } from "node:http";
import { createReadStream, existsSync, statSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const rootDir = normalize(join(__dirname, "..", "src"));
const host = process.env.UI_HOST || "127.0.0.1";
const port = Number(process.env.UI_PORT || "8080");

const MIME_TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".ts": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
};

function send(res, status, body, contentType = "text/plain; charset=utf-8") {
  res.writeHead(status, { "Content-Type": contentType });
  res.end(body);
}

function resolvePath(urlPath) {
  const sanitized = decodeURIComponent(urlPath.split("?")[0] || "/");
  const requested = sanitized === "/" ? "/index.html" : sanitized;
  const candidate = normalize(join(rootDir, requested));
  if (!candidate.startsWith(rootDir)) {
    return null;
  }
  if (existsSync(candidate) && statSync(candidate).isFile()) {
    return candidate;
  }
  return null;
}

const server = createServer((req, res) => {
  if (!req.url) {
    return send(res, 400, "missing request url");
  }

  const filePath = resolvePath(req.url);
  if (!filePath) {
    return send(res, 404, "not found");
  }

  const mimeType = MIME_TYPES[extname(filePath)] || "application/octet-stream";
  res.writeHead(200, { "Content-Type": mimeType });
  createReadStream(filePath).pipe(res);
});

server.listen(port, host, () => {
  console.log(`desktop UI dev server listening on http://${host}:${port}`);
});

function shutdown() {
  server.close(() => process.exit(0));
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
