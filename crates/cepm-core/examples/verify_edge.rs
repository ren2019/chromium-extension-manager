//! Explicit, one-shot runner for the same production Edge verification use case.
//! Usage: cargo run -p cepm-core --example verify_edge -- <CEPM data root> <installation id>
//! Close CEPM first so its file locks are released; browser authorization is still required.
use cepm_core::{
    Manager,
    browser::{BrowserCancellation, EdgeSession},
    profiles::ProfileBook,
};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Usage: verify_edge <CEPM data root> <installation id>".into());
    }
    let root = std::path::Path::new(&args[1]);
    let mut manager = Manager::open(root)?;
    let record = manager
        .list()?
        .into_iter()
        .find(|r| r.id == args[2])
        .ok_or("Installation not found")?;
    let book = ProfileBook::open(root)?;
    let profile = book.get(&record.target.profile)?;
    let evidence = {
        let mut session = EdgeSession::connect(profile, BrowserCancellation::default())?;
        session.verify(&record)?
    };
    let confirmed = manager.confirm_browser(evidence, &profile.target())?;
    println!(
        "{}",
        serde_json::json!({"installation":confirmed.id,"extensionId":confirmed.extension_id,"version":confirmed.effective_version,"verification":confirmed.verification,"disconnected":true})
    );
    Ok(())
}
