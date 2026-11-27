//! Open-from-a-link: decide whether a URL may be fetched (no I/O here; the app does the fetching).
//! Links are untrusted. Only https URLs on an allow-listed host (or the app's own origin) pass.

/// Hosts the app may fetch from besides its own origin (MakerFoundry).
pub const MAKERFOUNDRY_HOSTS: &[&str] = &["makerfoundry.app", "www.makerfoundry.app"];

/// Longest link we will look at (bytes).
pub const MAX_LINK_BYTES: usize = 2048;

/// What a link points at, from its file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Project,
    Stl,
    Obj,
}

impl LinkKind {
    /// The largest download we accept for this kind (bytes).
    pub fn max_bytes(self) -> usize {
        match self {
            LinkKind::Project => crate::file::MAX_FILE_BYTES,
            LinkKind::Stl | LinkKind::Obj => crate::mesh::MAX_BYTES,
        }
    }
}

/// A link that passed the checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenLink {
    pub url: String,
    pub kind: LinkKind,
}

/// Why a link was refused. Nothing is fetched in any of these cases.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("there is no link to open")]
    Missing,
    #[error("the link is too long or has odd characters")]
    Malformed,
    #[error("only secure (https) links can be opened")]
    NotHttps,
    #[error("links from {0} are not allowed; only MakerFoundry links open here")]
    HostNotAllowed(String),
    #[error("that link is not a .yarnfoundry, .stl or .obj file")]
    UnknownFileType,
}

/// Pulls the link out of a page query (`?open=<url>`) or a desktop link (`yarnfoundry://open?file=<url>`).
pub fn extract(input: &str) -> Result<String, LinkError> {
    let query = match input.split_once('?') {
        Some((_, q)) => q,
        None => input,
    };
    let query = query.split('#').next().unwrap_or("");
    let is_desktop = input.starts_with("yarnfoundry://");
    let key = if is_desktop { "file" } else { "open" };
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=')
            && k == key
        {
            return percent_decode(v).filter(|s| !s.is_empty()).ok_or(LinkError::Malformed);
        }
    }
    Err(LinkError::Missing)
}

/// Checks a URL against the allow-list plus `same_origin_host` (the host serving the app, if any).
pub fn check(url: &str, same_origin_host: Option<&str>) -> Result<OpenLink, LinkError> {
    if url.len() > MAX_LINK_BYTES || url.chars().any(|c| c.is_control() || c.is_whitespace() || c == '\\') {
        return Err(LinkError::Malformed);
    }
    let rest = match url.split_at_checked(8) {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https://") => rest,
        _ => return Err(LinkError::NotHttps),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return Err(LinkError::Malformed);
    }
    let host = match authority.rsplit_once(':') {
        Some((h, "443")) => h,
        Some(_) => return Err(LinkError::HostNotAllowed(authority.to_ascii_lowercase())),
        None => authority,
    };
    let host = host.to_ascii_lowercase();
    let allowed = MAKERFOUNDRY_HOSTS.contains(&host.as_str()) || same_origin_host.is_some_and(|h| !h.is_empty() && h.eq_ignore_ascii_case(&host));
    if host.is_empty() || !allowed {
        return Err(LinkError::HostNotAllowed(host));
    }
    let path = tail.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
    let kind = if path.ends_with(".yarnfoundry") {
        LinkKind::Project
    } else if path.ends_with(".stl") {
        LinkKind::Stl
    } else if path.ends_with(".obj") {
        LinkKind::Obj
    } else {
        return Err(LinkError::UnknownFileType);
    };
    Ok(OpenLink { url: url.to_string(), kind })
}

/// Extracts and checks in one step.
pub fn parse(input: &str, same_origin_host: Option<&str>) -> Result<OpenLink, LinkError> {
    check(&extract(input)?, same_origin_host)
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_makerfoundry_project_link_loads() {
        let l = parse("?open=https%3A%2F%2Fmakerfoundry.app%2Ff%2Fabc%2Fhat.yarnfoundry", None).unwrap();
        assert_eq!(l.kind, LinkKind::Project);
        assert_eq!(l.url, "https://makerfoundry.app/f/abc/hat.yarnfoundry");
    }

    #[test]
    fn figures_open_as_stl_or_obj() {
        assert_eq!(check("https://www.makerfoundry.app/x/bear.STL?v=1", None).unwrap().kind, LinkKind::Stl);
        assert_eq!(check("https://makerfoundry.app:443/x/bear.obj", None).unwrap().kind, LinkKind::Obj);
    }

    #[test]
    fn other_hosts_are_refused() {
        for u in [
            "https://evil.example/a.yarnfoundry",
            "https://makerfoundry.app.evil.example/a.stl",
            "https://makerfoundry.app@evil.example/a.stl",
            "https://makerfoundry.app:8443/a.stl",
            "https://evil.example/?x=makerfoundry.app/a.stl",
        ] {
            assert!(check(u, None).is_err(), "{u}");
        }
        assert_eq!(check("https://evil.example/a.stl", None), Err(LinkError::HostNotAllowed("evil.example".into())));
    }

    #[test]
    fn same_origin_is_allowed_and_only_that_origin() {
        assert!(check("https://app.yarnfoundry.app/s/a.yarnfoundry", Some("app.yarnfoundry.app")).is_ok());
        assert!(check("https://app.yarnfoundry.app/s/a.yarnfoundry", None).is_err());
    }

    #[test]
    fn insecure_and_odd_links_are_refused() {
        assert_eq!(check("http://makerfoundry.app/a.stl", None), Err(LinkError::NotHttps));
        assert_eq!(check("file:///etc/passwd", None), Err(LinkError::NotHttps));
        assert_eq!(check("javascript:alert(1)", None), Err(LinkError::NotHttps));
        assert_eq!(check("https://makerfoundry.app/a b.stl", None), Err(LinkError::Malformed));
        assert_eq!(check("https://makerfoundry.app/a.pdf", None), Err(LinkError::UnknownFileType));
        let long = format!("https://makerfoundry.app/{}.stl", "a".repeat(MAX_LINK_BYTES));
        assert_eq!(check(&long, None), Err(LinkError::Malformed));
    }

    #[test]
    fn desktop_links_use_the_same_allow_list() {
        let ok = parse("yarnfoundry://open?file=https%3A%2F%2Fmakerfoundry.app%2Fa.yarnfoundry", None).unwrap();
        assert_eq!(ok.kind, LinkKind::Project);
        assert!(parse("yarnfoundry://open?file=https%3A%2F%2Fevil.example%2Fa.yarnfoundry", None).is_err());
    }

    #[test]
    fn hostile_input_is_an_error_not_a_panic() {
        for s in ["", "?", "?open=", "?open=%", "?open=%zz", "?open=%ff%fe", "?other=1", "open=%E2%82", "yarnfoundry://open", "\u{0}"] {
            assert!(parse(s, None).is_err(), "{s:?}");
        }
        assert!(check("https://", None).is_err());
        assert!(check("https://é/a.stl", None).is_err());
        assert!(check("htt", None).is_err());
    }
}
