import http.server, urllib.request, urllib.error, os, itertools, threading
counter = itertools.count(1)
OUT = r'C:\Users\Public\Documents\relay'
os.makedirs(OUT, exist_ok=True)
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def do_POST(self):
        n = next(counter)
        ln = int(self.headers.get('Content-Length', 0) or 0)
        body = self.rfile.read(ln) if ln else b''
        open(os.path.join(OUT, '%04d_req.xml' % n), 'wb').write(body)
        req = urllib.request.Request('http://127.0.0.1:8090' + self.path, data=body, method='POST')
        for k, v in self.headers.items():
            if k.lower() not in ('host', 'content-length', 'connection'):
                req.add_header(k, v)
        try:
            with urllib.request.urlopen(req) as r:
                resp = r.read(); code = r.status; hdrs = list(r.headers.items())
        except urllib.error.HTTPError as e:
            resp = e.read(); code = e.code; hdrs = list(e.headers.items())
        except Exception as e:
            resp = str(e).encode(); code = 502; hdrs = [('Content-Type','text/plain')]
        open(os.path.join(OUT, '%04d_resp.xml' % n), 'wb').write(resp)
        self.send_response(code)
        for k, v in hdrs:
            if k.lower() not in ('transfer-encoding','content-length','connection'):
                self.send_header(k, v)
        self.send_header('Content-Length', str(len(resp)))
        self.end_headers()
        self.wfile.write(resp)
    def log_message(self, *a):
        pass
srv = http.server.ThreadingHTTPServer(('127.0.0.1', 8095), H)
srv.serve_forever()
