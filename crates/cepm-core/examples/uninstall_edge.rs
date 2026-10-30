//! Explicit controlled-acceptance runner for the production removal use case.
//! Usage: cargo run -p cepm-core --features edge-uninstall --example uninstall_edge -- <CEPM root> <installation id> --confirm-removal
//! Close CEPM first. This really removes the specified extension and its owned files.
use cepm_core::{
    Manager,
    browser::{AutomaticRemovalConfirmation, BrowserCancellation, EdgeSession},
    profiles::ProfileBook,
};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || args[3] != "--confirm-removal" {
        return Err("Usage: uninstall_edge <CEPM root> <installation id> --confirm-removal".into());
    }
    let root = std::path::Path::new(&args[1]);
    let mut manager = Manager::open(root)?;
    let book = ProfileBook::open(root)?;
    let record = manager
        .list()?
        .into_iter()
        .find(|r| r.id == args[2])
        .ok_or("Installation not found")?;
    let profile = book.get(&record.target.profile)?;
    let session = EdgeSession::connect(profile, BrowserCancellation::default())?;
    manager.uninstall_edge(
        &record.id,
        &profile.target(),
        record.generation,
        session,
        AutomaticRemovalConfirmation::Confirmed,
    )?;
    println!(
        "{}",
        serde_json::json!({"installation":record.id,"removed":true,"remainingManagedInstallations":manager.list()?.len(),"disconnected":true})
    );
    Ok(())
}
