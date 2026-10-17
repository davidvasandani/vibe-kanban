//! Server-rendered consent and error pages. They are deliberately static HTML
//! (no scripts) so they can carry a strict CSP and never be framed.

use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
};

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Headers for every page and redirect on the consent path.
pub fn harden(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn page(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
body{{font-family:system-ui,-apple-system,sans-serif;max-width:32rem;margin:3rem auto;padding:0 1.25rem;color:#111;line-height:1.5}}
.card{{border:1px solid #ddd;border-radius:12px;padding:1.5rem}}
h1{{font-size:1.3rem;margin-top:0}}
code{{background:#f4f4f4;padding:.1rem .3rem;border-radius:4px;word-break:break-all}}
.actions{{display:flex;gap:.75rem;margin-top:1.5rem}}
button{{flex:1;font-size:1rem;padding:.7rem;border-radius:8px;border:1px solid #111;cursor:pointer}}
.approve{{background:#111;color:#fff}}
.deny{{background:#fff;color:#111}}
.warn{{color:#8a4b00}}
</style></head><body><div class="card">{body}</div></body></html>"#
    )
}

pub fn error_page(status: StatusCode, message: &str) -> Response {
    let body = format!(
        "<h1>Authorization request rejected</h1><p>{}</p><p>Start the connection again from the application you were linking.</p>",
        html_escape(message)
    );
    harden((status, Html(page("Vibe Kanban authorization", &body))).into_response())
}

pub struct ConsentView<'a> {
    pub client_name: Option<&'a str>,
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub request_id: &'a str,
    pub consent: &'a str,
}

pub fn consent_page(view: ConsentView<'_>) -> Response {
    let name = view.client_name.unwrap_or("An unnamed application");
    let destination = url::Url::parse(view.redirect_uri)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| view.redirect_uri.to_string());
    let body = format!(
        r#"<h1>Connect {name} to Vibe Kanban?</h1>
<p><strong>{name}</strong> is asking for <strong>full access to the Vibe Kanban MCP tools</strong>.
That includes reading, creating, updating and deleting issues and workspaces, and running coding-agent sessions.</p>
<p>After you approve, you return to <code>{destination}</code>.</p>
<p class="warn">Only approve if you started this connection yourself just now.</p>
<p><small>Client ID: <code>{client_id}</code></small></p>
<form method="post" action="/oauth/authorize">
<input type="hidden" name="request_id" value="{request_id}">
<input type="hidden" name="consent" value="{consent}">
<div class="actions">
<button class="deny" type="submit" name="decision" value="deny">Deny</button>
<button class="approve" type="submit" name="decision" value="approve">Approve</button>
</div>
</form>"#,
        name = html_escape(name),
        destination = html_escape(&destination),
        client_id = html_escape(view.client_id),
        request_id = html_escape(view.request_id),
        consent = html_escape(view.consent),
    );
    harden(Html(page("Authorize application · Vibe Kanban", &body)).into_response())
}
