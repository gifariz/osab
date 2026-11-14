//! OSAB in the browser: the mesh step and the solver as a WebAssembly module, the browser's twin of
//! the desktop app's Tauri commands (`gui/src-tauri`) and of `osab api`. One call, JSON in, JSON out
//! (`osab_mesh::api::call`), so all three answer alike.
//!
//! No wasm-bindgen: the page writes the command and the model (UTF-8) into buffers from [`alloc`],
//! calls [`call`] and reads the reply at [`reply_ptr`] / [`reply_len`]. The glue is
//! `gui/src/ipc/wasm.ts`.
//!
//! A panic stops the module (WebAssembly has no unwinding here). The panic hook first leaves an error
//! reply of kind "crash" with the message, so the page can show it and load a fresh module. Running out
//! of memory stops the module without a message; the reply is then empty.

use std::cell::RefCell;
use std::sync::Once;

use osab_solver::api::ErrorReport;

thread_local! {
    /// The last reply, or the crash report of a panic, for the page to read.
    static REPLY: RefCell<String> = const { RefCell::new(String::new()) };
}

/// A buffer of `len` bytes for the page to write a string into; [`call`] takes it back.
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()).cast()
}

/// Runs `command` (`analyze`, `mesh` or `orient`) on the model and keeps the reply for [`reply_ptr`]
/// and [`reply_len`]. `max_joints` is the size limit for `analyze` (0: none).
///
/// # Safety
/// `cmd` and `input` must come from [`alloc`] with exactly these lengths; this call frees them.
#[no_mangle]
pub unsafe extern "C" fn call(
    cmd: *mut u8,
    cmd_len: usize,
    input: *mut u8,
    input_len: usize,
    max_joints: usize,
) {
    let cmd = take(cmd, cmd_len);
    let input = take(input, input_len);
    // An old reply would be read as this call's if the module stops without a message.
    REPLY.with(|r| *r.borrow_mut() = String::new());
    let reply = reply_to(
        &String::from_utf8_lossy(&cmd),
        &String::from_utf8_lossy(&input),
        max_joints,
    );
    REPLY.with(|r| *r.borrow_mut() = reply);
}

/// Where the last reply starts (UTF-8 JSON).
#[no_mangle]
pub extern "C" fn reply_ptr() -> *const u8 {
    REPLY.with(|r| r.borrow().as_ptr())
}

/// The last reply's length in bytes.
#[no_mangle]
pub extern "C" fn reply_len() -> usize {
    REPLY.with(|r| r.borrow().len())
}

/// The reply to one call: `{"ok": ...}` or `{"error": report}`.
pub fn reply_to(command: &str, input: &str, max_joints: usize) -> String {
    keep_panic_messages();
    osab_mesh::api::call(command, input, (max_joints > 0).then_some(max_joints))
}

unsafe fn take(ptr: *mut u8, len: usize) -> Box<[u8]> {
    Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len))
}

/// Makes a panic leave a "crash" reply before the module stops (then runs the default hook, which
/// prints the message in native builds and tests).
fn keep_panic_messages() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let report = ErrorReport::new("crash", format!("the solver stopped: {info}"));
            let text = osab_mesh::api::reply::<()>(Err(report));
            REPLY.with(|r| {
                if let Ok(mut r) = r.try_borrow_mut() {
                    *r = text;
                }
            });
            default(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTAL: &str = include_str!("../../tests/benchmarks/b05_portal_frame/model.json");

    /// The exported functions, used as the page uses them.
    fn exported(command: &str, input: &str, max_joints: usize) -> String {
        let write = |s: &str| {
            let p = alloc(s.len());
            unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), p, s.len()) };
            p
        };
        let (c, i) = (write(command), write(input));
        unsafe {
            call(c, command.len(), i, input.len(), max_joints);
            let bytes = std::slice::from_raw_parts(reply_ptr(), reply_len());
            String::from_utf8(bytes.to_vec()).unwrap()
        }
    }

    fn kind(reply: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(reply).unwrap();
        v["error"]["kind"].as_str().unwrap_or("ok").to_string()
    }

    #[test]
    fn answers_like_osab_api() {
        for command in osab_mesh::api::COMMANDS {
            // `spectrum` takes a spectrum function, `section` a shape, not a model.
            let input = match command {
                "spectrum" => r#"{"id": "RS", "points": [[0, 0.4], [1, 1]]}"#,
                "section" => {
                    r#"{"shape": {"type": "channel", "depth": 0.3, "width": 0.1, "flange": 0.012, "web": 0.008}}"#
                }
                _ => PORTAL,
            };
            let reply = exported(command, input, 0);
            assert_eq!(reply, osab_mesh::api::call(command, input, None));
            assert!(reply.starts_with("{\"ok\":"), "{command}: {reply}");
        }
    }

    #[test]
    fn errors_come_back_as_replies() {
        assert_eq!(kind(&exported("analyze", "{", 0)), "json");
        assert_eq!(kind(&exported("solve", PORTAL, 0)), "invalid");
        assert_eq!(kind(&exported("analyze", "", 0)), "json");
        // B05 has 4 joints.
        assert_eq!(kind(&exported("analyze", PORTAL, 4)), "ok");
        let big = exported("analyze", PORTAL, 3);
        assert_eq!(kind(&big), "too_big");
        assert!(
            big.contains("\"joints\":4") && big.contains("\"limit\":3"),
            "{big}"
        );
    }

    #[test]
    fn a_panic_leaves_a_crash_reply() {
        keep_panic_messages();
        let stopped = std::panic::catch_unwind(|| panic!("index out of bounds"));
        assert!(stopped.is_err());
        let reply = REPLY.with(|r| r.borrow().clone());
        assert_eq!(kind(&reply), "crash");
        assert!(reply.contains("index out of bounds"), "{reply}");
    }
}
