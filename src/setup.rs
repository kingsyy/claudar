/// The interactive browser-based setup wizard has been moved to the GUI application
/// (Phase 5). The CLI `setup` command is deprecated.
///
/// To authenticate manually, create a session file at
/// `~/.config/claude-notify/sessions/default.json` with the following structure:
///
/// ```json
/// {
///   "org_id": "<your-organization-id>",
///   "session_key": "<your-sessionKey-cookie>",
///   "full_cookie_string": "<full Cookie header value from browser dev tools>"
/// }
/// ```
///
/// You can find these values by:
/// 1. Opening claude.ai in your browser and logging in
/// 2. Opening DevTools (F12) → Application → Cookies
/// 3. Copying the `sessionKey` cookie value and the full Cookie header from any API request
pub fn run_setup(_verbose: bool, _instance_name: Option<String>) -> anyhow::Result<()> {
    println!("\n⚠️  The `setup` command is deprecated.");
    println!();
    println!("The interactive setup wizard will be available in the GUI app (coming soon).");
    println!();
    println!("To set up manually, create a session file at:");
    println!("  ~/.config/claude-notify/sessions/default.json");
    println!();
    println!("Required fields:");
    println!("  org_id            — your Anthropic organization UUID");
    println!("  session_key       — value of the sessionKey cookie from claude.ai");
    println!("  full_cookie_string — full Cookie header from any claude.ai API request");
    println!();
    println!("Find these values in your browser's DevTools:");
    println!("  F12 → Application → Cookies → claude.ai");
    println!("  F12 → Network → any /api/organizations/ request → Headers → Cookie");

    Ok(())
}
