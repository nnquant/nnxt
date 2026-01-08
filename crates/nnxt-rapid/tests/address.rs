use nnxt_rapid::{Address, AddressError};

#[test]
fn test_address_valid() {
    let addr = Address::new("market/ctp").expect("valid address");
    assert_eq!(addr.path(), "market/ctp");
}

#[test]
fn test_address_invalid_empty() {
    let err = Address::new("").unwrap_err();
    assert!(matches!(err, AddressError::Empty));
}

#[test]
fn test_address_invalid_segment() {
    let err = Address::new("market//ctp").unwrap_err();
    assert!(matches!(err, AddressError::EmptySegment));
}

#[test]
fn test_address_invalid_char() {
    let err = Address::new("market/ctp$bad").unwrap_err();
    assert!(matches!(err, AddressError::InvalidChar { .. }));
}
