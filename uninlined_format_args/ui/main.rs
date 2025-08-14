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

    info!(name: "test", "This is a test with {:?}", b);

    info!(name: "test", { b }, "This is a test with {}", a);

    info!(name: "test", target: "test_target", parent: "test_parent", { field1: "value1" }, "This is a test with {}", a);

    info!({ a, b }, "This is a test with {}", a);

    info!("This is a test with {:?}", b);

    info!({ a }, "This is a test with {b:?}");

    info!({ a, b }, "This is a test");

    info!(name: "test", target: "test_target", parent: "test_parent", { field1: "value1" }, "This is a test with {a}");
}
