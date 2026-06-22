//! Auth module — no-op stub.
//!
//! Future intent: validate a student subscription or API key before allowing
//! conversion. When monetisation is introduced, replace `check()` with a call
//! to the payment/licence service.
//!
//! For now this always returns `Ok(())` so the rest of the app is unaffected.

pub fn check() -> anyhow::Result<()> {
    // TODO: Validate payment / subscription token here.
    Ok(())
}
