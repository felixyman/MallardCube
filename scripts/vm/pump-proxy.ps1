$code = @'
using System;
using System.IO;
using System.Net;
using System.Text;
using System.Threading;

public class PumpProxy {
    static int counter = 0;
    static object logLock = new object();
    static string dir = @"C:\Users\Public\Documents\pumpproxy";
    public static void Start(int port, string targetBase) {
        Directory.CreateDirectory(dir);
        ServicePointManager.ServerCertificateValidationCallback = delegate { return true; };
        var listener = new HttpListener();
        listener.Prefixes.Add("http://127.0.0.1:" + port + "/");
        listener.Start();
        while (true) {
            var ctx = listener.GetContext();
            ThreadPool.QueueUserWorkItem(delegate(object o) { Handle((HttpListenerContext)o, targetBase); }, ctx);
        }
    }
    static void Handle(HttpListenerContext ctx, string targetBase) {
        int id;
        lock (logLock) { id = ++counter; }
        try {
            string path = ctx.Request.Url.PathAndQuery;
            var ms = new MemoryStream();
            ctx.Request.InputStream.CopyTo(ms);
            var reqBytes = ms.ToArray();
            File.WriteAllBytes(Path.Combine(dir, id.ToString("000") + "_req.xml"), reqBytes);
            var target = targetBase + path;
            var wr = (HttpWebRequest)WebRequest.Create(target);
            wr.Method = ctx.Request.HttpMethod;
            wr.Credentials = CredentialCache.DefaultCredentials;
            wr.PreAuthenticate = true;
            wr.ContentType = ctx.Request.ContentType;
            foreach (string h in ctx.Request.Headers.AllKeys) {
                if (h.Equals("Authorization", StringComparison.OrdinalIgnoreCase)) continue;
                if (h.Equals("Host", StringComparison.OrdinalIgnoreCase)) continue;
                if (h.Equals("Content-Length", StringComparison.OrdinalIgnoreCase)) continue;
                if (h.Equals("Connection", StringComparison.OrdinalIgnoreCase)) continue;
                try { wr.Headers[h] = ctx.Request.Headers[h]; } catch { }
            }
            wr.ContentLength = reqBytes.Length;
            using (var rs = wr.GetRequestStream()) { rs.Write(reqBytes, 0, reqBytes.Length); }
            HttpWebResponse resp = null;
            try { resp = (HttpWebResponse)wr.GetResponse(); }
            catch (WebException we) { resp = (HttpWebResponse)we.Response; }
            var respMs = new MemoryStream();
            resp.GetResponseStream().CopyTo(respMs);
            var respBytes = respMs.ToArray();
            File.WriteAllBytes(Path.Combine(dir, id.ToString("000") + "_resp.xml"), respBytes);
            ctx.Response.StatusCode = (int)resp.StatusCode;
            ctx.Response.ContentType = resp.ContentType;
            foreach (string h in resp.Headers.AllKeys) {
                if (h.Equals("Transfer-Encoding", StringComparison.OrdinalIgnoreCase)) continue;
                if (h.Equals("Content-Length", StringComparison.OrdinalIgnoreCase)) continue;
                if (h.Equals("Connection", StringComparison.OrdinalIgnoreCase)) continue;
                try { ctx.Response.Headers[h] = resp.Headers[h]; } catch { }
            }
            ctx.Response.ContentLength64 = respBytes.Length;
            ctx.Response.OutputStream.Write(respBytes, 0, respBytes.Length);
            ctx.Response.OutputStream.Close();
        } catch (Exception ex) {
            try {
                var msg = Encoding.UTF8.GetBytes("proxy error: " + ex.Message);
                ctx.Response.StatusCode = 500;
                ctx.Response.ContentLength64 = msg.Length;
                ctx.Response.OutputStream.Write(msg, 0, msg.Length);
                ctx.Response.OutputStream.Close();
            } catch { }
        }
    }
}
'@
Add-Type -TypeDefinition $code -Language CSharp
[PumpProxy]::Start(8090, 'https://localhost:8443')
