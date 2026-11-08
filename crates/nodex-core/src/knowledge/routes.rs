//! HTTP route normalization shared by back-end route declarations and front-end calls.
//!
//! Ported from CodeGraph (`crates/core/src/routes.rs`).

/// Normalizes a route or URL into a comparable shape: lower case, no scheme/host/query,
/// parameters collapsed to `{}`, no trailing slash.
///
/// `/api/Orders/{id:int}` → `/api/orders/{}`, `` `${base}/orders/${id}` `` → `{}/orders/{}`
pub fn normalize_route(raw: &str) -> String {
    let mut s = raw.trim().trim_matches('`').trim_matches('"').trim_matches('\'').to_string();
    // Strip scheme + host.
    if let Some(idx) = s.find("://") {
        let rest = &s[idx + 3..];
        s = match rest.find('/') {
            Some(p) => rest[p..].to_string(),
            None => "/".to_string(),
        };
    }
    // Strip query string / fragment.
    if let Some(q) = s.find(['?', '#']) {
        s.truncate(q);
    }
    let mut segments = Vec::new();
    for seg in s.split('/') {
        if seg.is_empty() {
            continue;
        }
        let is_param = seg.starts_with(':')
            || (seg.starts_with('{') && seg.ends_with('}'))
            || (seg.starts_with('<') && seg.ends_with('>'))
            || seg.starts_with("${")
            || seg == "*"
            || (seg.starts_with('[') && seg.ends_with(']') && !seg.contains("controller") && !seg.contains("action"));
        if is_param || seg.contains("${") || seg.contains('{') {
            // Whole or partially templated segment: `{id}`, `order-${id}`.
            segments.push("{}".to_string());
        } else {
            segments.push(seg.to_ascii_lowercase());
        }
    }
    let leading_param = raw.trim().trim_matches('`').starts_with("${");
    let joined = segments.join("/");
    if leading_param {
        // Already starts with `{}` (a base-URL variable).
        joined
    } else {
        format!("/{joined}")
    }
}

/// Joins a route prefix and a sub-path (`api/[controller]` + `{id}`).
pub fn join_route(prefix: &str, path: &str) -> String {
    let p = prefix.trim().trim_matches('/');
    let s = path.trim();
    if s.starts_with('/') && p.is_empty() || s.starts_with('~') {
        return format!("/{}", s.trim_start_matches('~').trim_matches('/'));
    }
    let s = s.trim_matches('/');
    match (p.is_empty(), s.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{s}"),
        (false, true) => format!("/{p}"),
        (false, false) => format!("/{p}/{s}"),
    }
}

/// ASP.NET: a leading `/` or `~/` on an action route overrides the controller prefix.
pub fn join_aspnet_route(prefix: &str, path: &str) -> String {
    let s = path.trim();
    if s.starts_with('/') || s.starts_with('~') {
        return format!("/{}", s.trim_start_matches('~').trim_matches('/'));
    }
    join_route(prefix, s)
}

/// True when a normalized URL (possibly starting with a `{}` base-URL variable) matches a
/// normalized route.
pub fn url_matches(url: &str, route: &str) -> bool {
    if url == route {
        return true;
    }
    // `{}/orders/{}` (a base URL variable) matches any route ending with `/orders/{}`.
    match url.strip_prefix("{}") {
        Some(tail) if tail.starts_with('/') && tail.len() > 1 => route.ends_with(tail),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_params_and_hosts() {
        assert_eq!(normalize_route("/api/Orders/{id:int}"), "/api/orders/{}");
        assert_eq!(normalize_route("/api/orders/:id/cancel"), "/api/orders/{}/cancel");
        assert_eq!(normalize_route("`/api/orders/${id}/cancel`"), "/api/orders/{}/cancel");
        assert_eq!(normalize_route("http://localhost:5000/api/orders?x=1"), "/api/orders");
        assert_eq!(normalize_route("`${API_BASE}/profile`"), "{}/profile");
        assert_eq!(normalize_route("/items/<int:item_id>"), "/items/{}");
        assert_eq!(normalize_route("/users/[id]"), "/users/{}");
        assert_eq!(normalize_route("/api/orders/"), "/api/orders");
    }

    #[test]
    fn joins_prefixes() {
        assert_eq!(join_route("api/orders", "{id}"), "/api/orders/{id}");
        assert_eq!(join_route("", "/health"), "/health");
        assert_eq!(join_route("/api", "/orders"), "/api/orders");
        assert_eq!(join_route("orders", ""), "/orders");
        assert_eq!(join_aspnet_route("api/orders", "/override"), "/override");
        assert_eq!(join_aspnet_route("api/orders", "~/override"), "/override");
        assert_eq!(join_aspnet_route("api/orders", "{id}"), "/api/orders/{id}");
    }

    #[test]
    fn matches_base_url_templates() {
        assert!(url_matches("/api/orders/{}", "/api/orders/{}"));
        assert!(url_matches("{}/api/orders", "/api/orders"));
        assert!(!url_matches("{}/orders", "/api/order"));
        assert!(!url_matches("{}", "/api/orders"));
    }
}
