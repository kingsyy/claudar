/// Minimal, GUI-free setup path — kept deliberately for headless and SSH use, where
/// the GUI wizard (`ui/src/routes/Wizard.svelte`, in-app webview auth) cannot run.
///
/// To authenticate manually, create a session file at
/// `~/.config/claudar/sessions/default.json` with the following structure:
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
    println!("\nClaudar — minimal setup (no GUI required).");
    println!();
    println!("This is the headless/SSH path: it tells you how to write the session file by hand.");
    println!("If you have a desktop session, the Claudar GUI app has an interactive setup wizard");
    println!("that logs you in and writes this file for you.");
    println!();
    println!("To set up manually, create a session file at:");
    println!("  ~/.config/claudar/sessions/default.json");
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
