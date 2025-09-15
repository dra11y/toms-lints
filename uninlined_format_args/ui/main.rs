#![allow(unused)]

mod __private {
    pub use std::format;
    pub use std::option::Option;

    pub fn mk_ident(id: &str, _span: Option<()>) -> String {
        id.to_string()
    }

    #[derive(Copy, Clone, Debug)]
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
    let d = 5.159317;
    let e = 255u8;
    let ptr = &a as *const i32;

    //~v uninlined_format_args
    info!(name: "test", "This is a test with {:?}", b);
    //~v uninlined_format_args
    info!(name: "test", { b }, "This is a test with {}", a);
    //~v uninlined_format_args
    info!(name: "test", target: "test_target", parent: "test_parent", { field1: "value1" }, "This is a test with {}", a);
    //~v uninlined_format_args
    info!({ a, b }, "This is a test with {}", a);
    //~v uninlined_format_args
    info!(
        "This is a test with {} and {:?}, with {} several {} placeholders {}",
        a, b, c, d, e
    );

    // Additional format variant examples
    //~v uninlined_format_args
    info!("Display format: {}", c);

    // Debug format
    //~v uninlined_format_args
    info!("Debug format: {:?}", c);

    // Scientific notation (lowercase)
    //~v uninlined_format_args
    info!("Scientific lower: {:e}", d);

    // Scientific notation (uppercase)
    //~v uninlined_format_args
    info!("Scientific upper: {:E}", d);

    // Octal format
    //~v uninlined_format_args
    info!("Octal: {:o}", e);

    // Pointer format
    //~v uninlined_format_args
    info!("Pointer: {:p}", ptr);

    // Binary format
    //~v uninlined_format_args
    info!("Binary: {:b}", e);

    // Hexadecimal (lowercase)
    //~v uninlined_format_args
    info!("Hex lower: {:x}", e);

    // Hexadecimal (uppercase)
    //~v uninlined_format_args
    info!("Hex upper: {:X}", e);

    // Complex format options
    //~v uninlined_format_args
    info!("Right aligned: {:>10}", c);
    //~v uninlined_format_args
    info!("Left aligned: {:<10}", c);
    //~v uninlined_format_args
    info!("Center aligned: {:^10}", c);
    //~v uninlined_format_args
    info!("Zero padded: {:08}", c);
    //~v uninlined_format_args
    info!("With sign: {:+}", c);
    //~v uninlined_format_args
    info!("Precision: {:.2}", d);
    //~v uninlined_format_args
    info!("Alternate hex: {:#x}", e);
    //~v uninlined_format_args
    info!("Debug hex: {:x?}", e);

    // Already inlined (should not lint)
    info!({ a }, "This is a test with {b:?}");
    info!({ a, b }, "This is a test");
    info!(name: "test", target: "test_target", parent: "test_parent", { field1: "value1" }, "This is a test with {a}");

    // Test mock of quote::format_ident! that wrongly renames vars to `arg`.
    let ident_first = "First";
    let ident_second = "Second";
    let ident_third = "Third";
    //~v uninlined_format_args
    let mock_ident = mock_format_ident!("{}{ident_second}{}", ident_first, ident_third);

    // Additional edge case tests for macro expansion parsing
    let with_comma = "has,comma";
    let with_quote = "has\"quote";
    //~v uninlined_format_args
    let mock_ident2 = mock_format_ident!("{}", with_comma);
    //~v uninlined_format_args
    let mock_ident3 = mock_format_ident!("{}{}", "literal,string", ident_first);

    // Test with raw strings
    //~v uninlined_format_args
    let mock_ident4 = mock_format_ident!("{}", r#"raw"string"#);

    // Test with function calls as arguments
    let mock_ident5 = mock_format_ident!("{}", format!("nested"));

    // Test multiple complex arguments
    //~v uninlined_format_args
    let mock_ident6 = mock_format_ident!(
        "{}{}{}",
        "literal_string",
        format!("literal_string{with_comma}"),
        ident_third
    );

    // Define a test struct and function for the edge cases
    #[derive(Debug)]
    struct SomeStruct {
        field1: String,
        field2: Vec<i32>,
        field3: i32,
    }

    fn some_function(a: i32, b: i32, c: i32) -> i32 {
        a + b + c
    }

    // Additional edge case variables
    let var1 = "test";
    let var2 = 42;
    let var3 = "with,comma";
    let var4 = "with\"quote";
    let var5 = "with\\backslash";
    let complex_var = vec![1, 2, 3];

    // 1. String literals with commas inside them
    //~v uninlined_format_args
    mock_format_ident!("{}", "literal,with,commas");

    // 2. String literals with escaped quotes
    //~v uninlined_format_args
    mock_format_ident!("{}", "literal\"with\"quotes");

    // 3. String literals with both escaped quotes and commas
    //~v uninlined_format_args
    mock_format_ident!("{}", "complex\"literal,with\"everything");

    // 4. Raw string literals
    //~v uninlined_format_args
    mock_format_ident!("{}", r#"raw"string"with"quotes"#);

    // 5. Complex expressions as arguments
    mock_format_ident!("{}", format!("nested,format,call"));

    // 6. Function calls with commas in arguments
    mock_format_ident!("{}", some_function(1, 2, 3));

    // 7. Macro calls as arguments
    mock_format_ident!("{:?}", vec![1, 2, 3]);

    // 8. Complex nested structures
    mock_format_ident!(
        "{:?}",
        SomeStruct {
            field1: "value,with,comma".to_string(),
            field2: vec![1, 2, 3],
            field3: 42,
        }
    );

    // 9. Multiple format placeholders with complex args
    mock_format_ident!(
        "{}{:?}{:?}",
        format!("first,arg"),
        vec![1, 2, 3],
        SomeStruct {
            field1: "test,value".to_string(),
            field2: vec![4, 5],
            field3: 100,
        }
    );

    // 10. Positional parameters
    let name = "abc";
    let width = 10;
    //~v uninlined_format_args
    let _formatted = format!("[{:^1$}]", name, width);

    // 11. r#type should suggest {type:?} (remove r# prefix)
    //~vvv uninlined_format_args
    let r#type: &'static str = "test";
    let args = "arguments";
    println!("[{:?}] {args}", r#type);

    // Test cases for frivolous reassignments (ident = ident patterns)
    let val = 42;
    let value = "test";
    let name = "example";
    let config = SomeStruct {
        field1: "config".to_string(),
        field2: vec![1, 2],
        field3: 99,
    };

    // SHOULD LINT: frivolous reassignments (ident = ident)
    //~v uninlined_format_args
    info!("hello {val}", val = val);
    //~v uninlined_format_args
    info!("hello {value}", value = val);
    //~v uninlined_format_args
    info!("hello {name}", name = value);
    //~v uninlined_format_args
    info!("hello {val} and {value}", val = val, value = name);
    //~v uninlined_format_args
    println!("display {x}", x = val);
    //~v uninlined_format_args
    println!("display {x}, {value:?}", value = vec![1, 2, 3], x = val);
    //~v uninlined_format_args
    format!("debug {item:?}", item = value);

    // The following SHOULD lint because it would NOT result in duplicate placeholders
    //~v uninlined_format_args
    format!(
        "debug {item:?} {value2:?} {result}",
        result = some_function(1, 2, 3),
        item = value,
        value2 = vec![1, 2, 3]
    );

    // The following should NOT lint because it would result in duplicate placeholders
    format!(
        "debug {item:?} {value:?} {result}",
        result = some_function(1, 2, 3),
        item = value,
        value = vec![1, 2, 3]
    );

    // SHOULD NOT LINT: not frivolous reassignments
    info!("hello {value}", value = some_function(1, 2, 3));
    info!("hello {value}", value = config.field1);
    info!("hello {value}", value = config.field3);
    info!("hello {value}", value = "literal string");
    info!("hello {value}", value = 42);
    info!("hello {value}", value = format!("nested"));
    info!("hello {value:?}", value = vec![1, 2, 3]);
    info!("hello {value:?}", value = &val);
    unsafe {
        info!("hello {value}", value = *ptr);
    }

    // Edge cases that should NOT lint
    info!("hello {value}", value = val.to_string());
    info!("hello {value}", value = val + 1);
    info!("hello {value}", value = (val)); // parentheses make it not a simple ident
}
