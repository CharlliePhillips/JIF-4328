fn main() {
    let arg1 = std::env::args().nth(1).expect("no arg1 given");
    let arg2 = std::env::args().nth(2).expect("no arg2 given");

    println!("pattern: {:?}, path: {:?}", arg1, arg2)
}
