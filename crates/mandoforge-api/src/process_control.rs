use tokio::process::Command;

#[cfg(unix)]
pub(crate) fn configure_process_group(process: &mut Command) {
    use std::os::unix::process::CommandExt;

    process.as_std_mut().process_group(0);
}

#[cfg(not(unix))]
pub(crate) fn configure_process_group(_process: &mut Command) {}

#[cfg(unix)]
pub(crate) fn terminate_process_group(process_group_id: u32) {
    let Ok(process_group_id) = i32::try_from(process_group_id) else {
        return;
    };
    // SAFETY: a negative PID targets only the child-created process group.
    unsafe {
        libc::kill(-process_group_id, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
pub(crate) fn terminate_process_group(_process_group_id: u32) {}
