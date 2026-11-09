//! YarnFoundry render: turns a chart into SVG, PNG and PDF (filled in by later tasks).

/// Crate marker so the layer exists from the start.
pub fn crate_name() -> &'static str {
    "yarnfoundry-render"
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_itself() {
        assert_eq!(super::crate_name(), "yarnfoundry-render");
    }
}
