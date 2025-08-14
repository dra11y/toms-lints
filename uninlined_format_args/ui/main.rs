mod __private {
    pub use std::format;
    pub use std::option::Option;

    pub fn mk_ident(id: &str, _span: Option<()>) -> String {
        id.to_string()
    }

    #[derive(Copy, Clone)]
    pub struct IdentFragmentAdapter<T>(pub T);

    impl<T> IdentFragmentAdapter<T> {
        pub fn span(&self) -> Option<()> {
            None
        }
    }

    impl<T: std::fmt::Display> std::fmt::Display for IdentFragmentAdapter<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Display::fmt(&self.0, f)
        }
    }
}

macro_rules! mock_format_ident {
    ($fmt:expr) => {
        mock_format_ident_impl!([
            __private::Option::None,
            $fmt
        ])
    };

    ($fmt:expr, $($rest:tt)*) => {
        mock_format_ident_impl!([
            __private::Option::None,
            $fmt
        ] $($rest)*)
    };
}

macro_rules! mock_format_ident_impl {
    ([$span:expr, $($fmt:tt)*]) => {
        __private::mk_ident(
            &__private::format!($($fmt)*),
            $span,
        )
    };

    ([$span:expr, $($fmt:tt)*] $name:ident = $arg:expr) => {
        mock_format_ident_impl!([$span, $($fmt)*] $name = $arg,)
    };
    ([$span:expr, $($fmt:tt)*] $name:ident = $arg:expr, $($rest:tt)*) => {
        match __private::IdentFragmentAdapter(&$arg) {
            arg => mock_format_ident_impl!([$span, $($fmt)*, $name = arg] $($rest)*),
        }
    };

    ([$span:expr, $($fmt:tt)*] $arg:expr) => {
        mock_format_ident_impl!([$span, $($fmt)*] $arg,)
    };
    ([$span:expr, $($fmt:tt)*] $arg:expr, $($rest:tt)*) => {
        match __private::IdentFragmentAdapter(&$arg) {
            arg => mock_format_ident_impl!([$span, $($fmt)*, arg] $($rest)*),
        }
    };
}

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

    // Test mock of quote::format_ident! that wrongly renames `my_var_name` as `arg`.
    let my_var_name = "Test";
    let patch_name = mock_format_ident!("{}Patch", my_var_name);

    let user_provided_name = "Widget";
    let generated_ident = mock_format_ident!("Generated{}", user_provided_name);

    // Original examples testing tracing-like macro patterns
    info!(name: "test", "This is a test with {:?}", b);
    info!(name: "test", { b }, "This is a test with {}", a);
    info!(name: "test", target: "test_target", parent: "test_parent", { field1: "value1" }, "This is a test with {}", a);
    info!({ a, b }, "This is a test with {}", a);
    info!(
        "This is a test with {} and {:?}, with {} several {} placeholders {}",
        a, b, c, d, e
    );

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
