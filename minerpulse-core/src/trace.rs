use std::sync::OnceLock;

type TraceHook = fn(category: &str, event: &str, detail: &str);

static HOOK: OnceLock<TraceHook> = OnceLock::new();

/// Desktop sets this once at startup to capture driver/read diagnostics.
pub fn set_trace_hook(hook: TraceHook) {
    let _ = HOOK.set(hook);
}

pub fn trace(category: &str, event: &str, detail: &str) {
    if let Some(hook) = HOOK.get() {
        hook(category, event, detail);
    }
}
