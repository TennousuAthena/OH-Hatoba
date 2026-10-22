#!/usr/bin/env python3
"""Loopback generated-data HTTP fixture. No host files, real accounts or request logs."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, code, value=None, content_type="application/json", headers=None):
        data = value if isinstance(value, bytes) else json.dumps(value).encode() if value is not None else b""
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        for key, val in (headers or {}).items():
            self.send_header(key, val)
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/v1/models":
            self.reply(200, {"object": "list", "data": [{"id": "generated-fixture", "object": "model"}]})
        else:
            self.reply(405)

    def do_DELETE(self):
        self.reply(200)

    def do_POST(self):
        size = int(self.headers.get("Content-Length", "0"))
        if size < 0 or size > 1024 * 1024:
            self.reply(413)
            return
        try:
            body = json.loads(self.rfile.read(size))
        except (ValueError, UnicodeError):
            self.reply(400)
            return
        if self.path == "/v1/chat/completions":
            chunks = [
                {"id": "generated", "choices": [{"index": 0, "delta": {"role": "assistant", "content": "generated 中文 "}, "finish_reason": None}]},
                {"id": "generated", "choices": [{"index": 0, "delta": {"content": "🌊"}, "finish_reason": None}]},
                {"id": "generated", "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]},
            ]
            data = "".join("data: " + json.dumps(chunk, ensure_ascii=False) + "\n\n" for chunk in chunks) + "data: [DONE]\n\n"
            self.reply(200, data.encode(), "text/event-stream")
        elif self.path == "/mcp":
            if "id" not in body:
                self.reply(202)
                return
            method = body.get("method")
            headers = None
            if method == "initialize":
                result = {"protocolVersion": "2025-03-26", "capabilities": {"tools": {}}, "serverInfo": {"name": "generated-fixture", "version": "1.0"}}
                headers = {"mcp-session-id": "generated-session"}
            elif method == "tools/list":
                result = {"tools": [{"name": "echo", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}
            elif method == "tools/call" and body.get("params", {}).get("name") == "echo":
                text = body.get("params", {}).get("arguments", {}).get("text", "")
                result = {"content": [{"type": "text", "text": str(text)[:1024]}]}
            elif method == "ping":
                result = {}
            else:
                self.reply(200, {"jsonrpc": "2.0", "id": body["id"], "error": {"code": -32601, "message": "Unknown fixture method"}})
                return
            self.reply(200, {"jsonrpc": "2.0", "id": body["id"], "result": result}, headers=headers)
        else:
            self.reply(404)


if __name__ == "__main__":
    print("Disposable generated-data HTTP fixture ready on 28080", flush=True)
    ThreadingHTTPServer(("127.0.0.1", 28080), Handler).serve_forever()
