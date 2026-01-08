use std::mem::ManuallyDrop;
use std::time::Duration;

use nnxt_rapid::{cleanup, Address, Reader, Writer};

#[test]
fn test_spmc_fork() {
    let path = format!("test/spmc/{}", std::process::id());
    let addr = Address::new(&path).expect("address");
    let _ = cleanup(&addr);

    let mut writer = ManuallyDrop::new(Writer::<u64>::create(&addr, 16).expect("writer"));
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork failed");

    if pid == 0 {
        let mut reader = Reader::<u64>::connect(&addr).expect("reader");
        let value = reader.read_timeout(Duration::from_secs(1));
        assert_eq!(value.copied(), Some(42));
        unsafe {
            libc::_exit(0);
        }
    }

    writer.write(42);

    let mut status: libc::c_int = 0;
    let wait_rc = unsafe { libc::waitpid(pid, &mut status, 0) };
    assert_eq!(wait_rc, pid);
    assert!(libc::WIFEXITED(status));
    assert_eq!(libc::WEXITSTATUS(status), 0);

    unsafe {
        ManuallyDrop::drop(&mut writer);
    }
}
