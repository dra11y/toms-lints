fn main() {
    let x = 42; // This should trigger the lint
    let y = 10; // Another end-of-line comment

    // This comment is fine - it's on its own line
    let z = x + y; // But this comment is not

    if x > 0 {
        // This is fine - comment on its own line
        println!("x is positive");
    }

    // This is also fine
    if y > 0 {
        println!("y is positive"); // This should trigger the lint
    }

    let result = match x {
        // This is fine - comment on its own line
        42 => "magic number", /* block comment should not be here */
        _ => "other",
    };

    println!("{}", result);
}

struct Point {
    x: i32, // Field comment - should trigger lint
    y: i32, // Another field comment - should trigger lint
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        // This comment is fine - it's on its own line
        Self { x, y }
    }

    // This method comment is fine
    fn distance(&self) -> f64 {
        ((self.x * self.x + self.y * self.y) as f64).sqrt() // Should trigger lint
    }
}
