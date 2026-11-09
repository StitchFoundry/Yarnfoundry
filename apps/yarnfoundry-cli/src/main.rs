//! YarnFoundry command line (the chart commands arrive with the core features).

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        None | Some("--version") | Some("version") => println!("{}", yarnfoundry_core::banner()),
        Some(other) => {
            eprintln!("unknown command: {other}\nusage: yarnfoundry-cli [--version]");
            std::process::exit(2);
        }
    }
}
