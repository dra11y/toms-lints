#[macro_use]
mod simple_tracing_like {
    macro_rules! info {
        ($($arg:tt)+) => (
            println!($($arg)*)
        );
    }
}

fn main() {
    let a = 42;
    let b = "test";

    // This should lint (default behavior)
    info!("Normal case: {}", a);

    // Test module-level allow
    {
        #[allow(uninlined_format_args)]
        {
            info!("Should not lint due to allow: {}", a);
        }
    }

    // Test statement-level allow
    #[allow(uninlined_format_args)]
    info!("Should not lint due to statement allow: {}", b);

    // Test deny (should still lint with error level)
    #[deny(uninlined_format_args)]
    info!("Should lint with deny level: {}", a);

    // Test warn (should lint with warning level)
    #[warn(uninlined_format_args)]
    info!("Should lint with warn level: {}", b);

    // Already inlined (should never lint regardless of attributes)
    #[allow(uninlined_format_args)]
    info!("Already inlined: {a}");
}

// Test crate-level attributes
#[allow(uninlined_format_args)]
fn allowed_function() {
    let x = 123;
    info!("Should not lint in allowed function: {}", x);
}

#[deny(uninlined_format_args)]
fn denied_function() {
    let y = 456;
    info!("Should lint with deny in denied function: {}", y);
}
