use super::*;

#[test]
fn reads_one_byte_range() {
    let bytes = |start, end| Range::Bytes { start, end };
    assert_eq!(range("bytes=0-", 10), bytes(0, 10));
    assert_eq!(range("bytes=2-4", 10), bytes(2, 5));
    assert_eq!(range("bytes=0-18446744073709551615", 10), bytes(0, 10));
    assert_eq!(range("bytes=2-40", 10), bytes(2, 10));
    assert_eq!(range("bytes=-3", 10), bytes(7, 10));
    assert_eq!(range("bytes=-30", 10), bytes(0, 10));
    assert_eq!(range(" bytes= 9-9 ", 10), bytes(9, 10));
}

#[test]
fn refuses_a_range_past_the_end() {
    assert_eq!(range("bytes=10-", 10), Range::Unsatisfiable);
    assert_eq!(range("bytes=10-12", 10), Range::Unsatisfiable);
    assert_eq!(range("bytes=-0", 10), Range::Unsatisfiable);
    assert_eq!(range("bytes=0-", 0), Range::Unsatisfiable);
}

#[test]
fn ignores_what_it_does_not_take() {
    for header in [
        "items=0-1",
        "bytes=0-1,4-5",
        "bytes=4-2",
        "bytes=-",
        "bytes=a-b",
        "bytes=5",
    ] {
        assert_eq!(range(header, 10), Range::Ignored, "{header}");
    }
}
