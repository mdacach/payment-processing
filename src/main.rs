fn main() {
    println!("Hello, world!");
}

#[test]
fn oh_yes() {
    assert_eq!(2 + 2, 4);
}

#[test]
fn oh_no() {
    assert_eq!(2 + 2, 5);
}
