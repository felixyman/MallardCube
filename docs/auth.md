# Authentication

MallardCube authenticates at the HTTP layer and maps the authenticated identity
to **roles**, which then enforce row-level (RLS) and object-level (OLS)
security. There are two supported mechanisms, and the recommendation is to
delegate authentication to the edge (reverse proxy) — that is how SSAS-alike
BI servers are run in practice, because Excel/Power BI authenticate via
Windows/Basic at the HTTP layer, not via an app login flow.

> **Fail closed.** Every path below denies access when the identity cannot be
> established — there is no fallback to full access.

## 1. Trusted reverse proxy (recommended)

Run an authenticating reverse proxy (IIS, nginx, Envoy, `oauth2-proxy`,
Authelia, Entra App Proxy, …) in front of MallardCube. The proxy terminates
TLS, authenticates the user (Windows Auth / OIDC / LDAP / SAML), and injects a
header containing the authenticated user id. MallardCube trusts that header.

```jsonc
// proxy-config.json
{
  "auth": {
    "trusted_proxy": true,
    "trusted_header": "X-User"   // default
  },
  "roles": [
    {
      "name": "Analysts",
      "model_permission": "read",
      "members": [{ "member_name": "alice@example.com", "member_type": "user" }],
      "table_permissions": [
        { "table": "sales_fact", "filter_expression": "territory = 'North'" }
      ]
    }
  ]
}
```

Example — nginx in front of MallardCube, terminating TLS and setting the
header from a verified identity:

```nginx
server {
    listen 443 ssl;
    server_name cube.example.com;

    # TLS terminated here (required for the header to be trustworthy).

    location /xmla {
        proxy_pass http://127.0.0.1:8080;

        # Overwrite the header from the authenticated identity — never forward
        # the client-supplied header, or a client could spoof any user.
        proxy_set_header X-User $remote_user;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

### Security notes (trusted proxy)

- **The header is only as trustworthy as the link.** Always terminate TLS at
  the proxy and overwrite (never forward) the header from the client.
- A missing header is **deny-all** (`fail closed`).

## 2. OIDC (JWT Bearer) — no reverse proxy needed

For standalone deployments, MallardCube can validate an OIDC access token
itself. Send `Authorization: Bearer <token>` from the client; the proxy
validates the JWT signature against the IdP's JWKS and maps claims to identity
+ groups.

```jsonc
{
  "auth": {
    "oidc": {
      "issuer": "https://login.microsoftonline.com/{tenant}/v2.0",
      "audience": "<client-id>",
      // "jwks_uri": "https://.../discovery/v2.0/keys",   // optional; discovered if omitted
      "user_claim": "preferred_username",                // default "sub"
      "group_claim": "groups",                           // optional (array or string)
      "role_claim": "roles"                              // optional
    }
  }
}
```

How it works:

1. The JWT header's `kid` selects the signing key; keys are fetched from the
   JWKS and cached (key rotation triggers a single refresh).
2. The signature, `iss`, `aud`, `exp`, and `nbf` are validated.
3. `user_claim` (falling back to `sub`) becomes the user id; `group_claim` and
   `role_claim` become the group list.
4. The user id + groups resolve against `roles[].members` exactly like the
   trusted-proxy path, so RLS/OLS behave identically.

Supported algorithms: RS256/384/512, ES256/384, HS256/384/512 (from the JWKS).
A missing, expired, or invalid token is **deny-all**.

## Role resolution

Both mechanisms converge on the same resolver. A user is granted a role when
their identity matches a `members` entry (by user id or group):

- `model_permission: "none"` → deny everything.
- `model_permission: "read"` → row-level security via `table_permissions[].filter_expression`.
- `model_permission: "administrator"` → bypasses RLS/OLS.

Multiple roles union (most permissive wins). With no `auth` configured, the
proxy runs in admin-default mode (no authentication, full access) — this is
for local development only.
