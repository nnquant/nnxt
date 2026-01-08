use nnxt_rapid::{cleanup, Address, Reader, Writer};

#[test]
fn test_single_thread_read_write() {
    let path = format!("test/single/{}", std::process::id());
    let addr = Address::new(&path).expect("address");
    let _ = cleanup(&addr);

    let mut writer = Writer::<u64>::create(&addr, 8).expect("writer");
    let mut reader = Reader::<u64>::connect(&addr).expect("reader");

    writer.write(10);
    writer.write(20);
    writer.write(30);

    assert_eq!(*reader.read(), 10);
    assert_eq!(*reader.read(), 20);
    assert_eq!(*reader.read(), 30);
}
