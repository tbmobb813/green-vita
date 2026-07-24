use vita_newlib_shims as _;

mod api;
mod api_xbox;
mod app;
mod i18n;
mod input;
mod jobs;
mod safe_memory;
mod settings;
mod shell;
mod streaming;

use api_xbox::api::{
    ApiClient, ApiClientConfig, Console, ConsolesResponse, StreamKind, WaitTimeResponse,
};
use api_xbox::auth::{DeviceCodeAuth, DeviceCodePoll, MsalAuth, StreamingCredentials, XboxProfile};
use api_xbox::stream::{Stream, StreamState};
use app::{App, AppCommand, AppState, InputCommand, NavigationCommand};
use settings::Locale;

#[used]
#[unsafe(export_name = "sceUserMainThreadStackSize")]
pub static SCE_USER_MAIN_THREAD_STACK_SIZE: u32 = 4 * 1024 * 1024;

#[used]
#[unsafe(export_name = "sceLibcHeapSize")]
pub static SCE_LIBC_HEAP_SIZE: u32 = 40 * 1024 * 1024;

#[used]
#[unsafe(export_name = "_newlib_heap_size_user")]
pub static NEWLIB_HEAP_SIZE_USER: u32 = 192 * 1024 * 1024;

mod fs_utils {
    use anyhow::{Context, Result};

    /// Writes to a `.tmp` sibling and renames it over `path`, so a crash or power loss
    /// mid-write can't leave `path` truncated or missing. `std::fs::write` alone doesn't
    /// reliably truncate an existing file on the Vita's newlib filesystem, which is why
    /// the previous version of this function removed `path` first - but that ordering
    /// left a window where a crash between the removal and the write lost the file
    /// entirely (notably the Microsoft refresh token in settings.json).
    pub fn write_file_truncating(path: &str, data: impl AsRef<[u8]>) -> Result<()> {
        let tmp_path = format!("{path}.tmp");
        let _ = std::fs::remove_file(&tmp_path);
        std::fs::write(&tmp_path, data)
            .with_context(|| format!("failed to write {tmp_path}"))?;
        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("failed to rename {tmp_path} to {path}"))
    }
}

const CRASH_LOG_DIR: &str = "ux0:data/green-vita";
const CRASH_LOG_PATH: &str = "ux0:data/green-vita/crash.log";

/// A panic or a fatal startup/runtime error otherwise unwinds silently back to
/// LiveArea with nothing visible to the user. This is the only diagnostic trail
/// left behind, so it's worth writing even though nothing here can surface it in
/// the UI.
fn write_crash_log(text: &str) {
    if std::fs::create_dir_all(CRASH_LOG_DIR).is_ok() {
        let _ = fs_utils::write_file_truncating(CRASH_LOG_PATH, text);
    }
}

fn run() -> anyhow::Result<()> {
    let _app_util = safe_memory::AppUtil::initialize()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let app = App::new()?;
        shell::run(app).await
    })
}

fn main() -> anyhow::Result<()> {
    std::panic::set_hook(Box::new(|info| write_crash_log(&info.to_string())));

    let result = run();
    if let Err(error) = &result {
        write_crash_log(&format!("{error:#}"));
    }
    result
}
