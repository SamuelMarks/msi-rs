pub fn foo() {
    match std::env::var("XYZ") {
        Ok(_) => {},
        Err(_) => unreachable!(),
    }
}
