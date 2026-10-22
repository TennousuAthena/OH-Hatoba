#!/usr/bin/env python3
"""Disposable loopback HTTPS fixture with an intentionally untrusted certificate."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import ssl
import subprocess
from tempfile import TemporaryDirectory


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        payload = b"generated certificate-rejection fixture\n"
        self.send_response(200)
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_args):
        pass


with TemporaryDirectory(prefix="oh-hatoba-tls-fixture-") as temp:
    key, cert = Path(temp) / "key.pem", Path(temp) / "cert.pem"
    subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", str(key), "-out", str(cert), "-days", "1", "-subj", "/CN=127.0.0.1", "-addext", "subjectAltName=IP:127.0.0.1"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    key.chmod(0o600)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(cert, key)
    server = ThreadingHTTPServer(("127.0.0.1", 28443), Handler)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print("Disposable untrusted TLS fixture ready on loopback 28443", flush=True)
    server.serve_forever()
