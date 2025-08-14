#[macro_use]
mod simple_tracing_like {
    // Test a few cases from tracing with complex patterns.
    // Complete coverage requires lintcheck CI test.
    macro_rules! info {
        // Name / target / parent.
        (name: $name:expr, target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
            println!($($arg)*)
        );

        // Name.
        (name: $name:expr, { $($field:tt)* }, $($arg:tt)* ) => (
            println!($($arg)*)
        );
        (name: $name:expr, $($arg:tt)+ ) => (
            println!($($arg)*)
        );

        // ...
        ({ $($field:tt)+ }, $($arg:tt)+ ) => (
            println!($($arg)*)
        );
        ($($arg:tt)+) => (
            println!($($arg)*)
        );
    }
}

fn main() {
    let a = 1;
    let b = Some("test");
    let c = 42;
    let d = 3.14159;
    let e = 255u8;
    let ptr = &a as *const i32;

    // Original examples testing tracing-like macro patterns
    info!(name: "test", "This is a test with {:?}", b);
    info!(name: "test", { b }, "This is a test with {}", a);
    info!(name: "test", target: "test_target", parent: "test_parent", { field1: "value1" }, "This is a test with {}", a);
    info!({ a, b }, "This is a test with {}", a);
    info!("This is a test with {:?}", b);

    // Additional format variant examples
    // Display format (default)
    info!("Display format: {}", c);

    // Debug format
    info!("Debug format: {:?}", c);

    // Scientific notation (lowercase)
    info!("Scientific lower: {:e}", d);

    // Scientific notation (uppercase)
    info!("Scientific upper: {:E}", d);

    // Octal format
    info!("Octal: {:o}", e);

    // Pointer format
    info!("Pointer: {:p}", ptr);

    // Binary format
    info!("Binary: {:b}", e);

    // Hexadecimal (lowercase)
    info!("Hex lower: {:x}", e);

    // Hexadecimal (uppercase)
    info!("Hex upper: {:X}", e);

    // Complex format options
    info!("Right aligned: {:>10}", c);
    info!("Left aligned: {:<10}", c);
    info!("Center aligned: {:^10}", c);
    info!("Zero padded: {:08}", c);
    info!("With sign: {:+}", c);
    info!("Precision: {:.2}", d);
    info!("Alternate hex: {:#x}", e);
    info!("Debug hex: {:x?}", e);

    // Already inlined (should not lint)
    info!({ a }, "This is a test with {b:?}");
    info!({ a, b }, "This is a test");
    info!(name: "test", target: "test_target", parent: "test_parent", { field1: "value1" }, "This is a test with {a}");
}
