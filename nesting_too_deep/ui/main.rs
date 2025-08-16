fn test_if_else_nesting() {
    let value = 42;
    let option = Some(10);
    let result: Result<i32, &str> = Ok(5);

    // Level 1 - OK
    if value > 0 {
        println!("Level 1");
    }

    // Level 2 - OK
    if value > 0 {
        if value < 100 {
            println!("Level 2");
        }
    }

    // Level 3 - Should trigger lint
    if value > 0 {
        if value < 100 {
            if value % 2 == 0 {
                println!("Level 3 - should lint");
            }
        }
    }

    // Level 4 - Should definitely trigger lint
    if value > 0 {
        if value < 100 {
            if value % 2 == 0 {
                if value > 10 {
                    println!("Level 4 - deep nesting");
                }
            }
        }
    }
}

fn test_if_let_nesting() {
    let opt1 = Some(42);
    let opt2 = Some(10);
    let opt3 = Some(5);

    // Level 1 - OK
    if let Some(x) = opt1 {
        println!("Level 1: {}", x);
    }

    // Level 2 - OK
    if let Some(x) = opt1 {
        if let Some(y) = opt2 {
            println!("Level 2: {} {}", x, y);
        }
    }

    // Level 3 - Should trigger lint
    if let Some(x) = opt1 {
        if let Some(y) = opt2 {
            if let Some(z) = opt3 {
                println!("Level 3 - should lint: {} {} {}", x, y, z);
            }
        }
    }
}

fn test_result_nesting() {
    let res1: Result<i32, &str> = Ok(42);
    let res2: Result<i32, &str> = Ok(10);
    let res3: Result<i32, &str> = Ok(5);

    // Level 1 - OK
    if let Ok(x) = res1 {
        println!("Level 1: {}", x);
    }

    // Level 2 - OK
    if let Ok(x) = res1 {
        if let Ok(y) = res2 {
            println!("Level 2: {} {}", x, y);
        }
    }

    // Level 3 - Should trigger lint
    if let Ok(x) = res1 {
        if let Ok(y) = res2 {
            if let Ok(z) = res3 {
                println!("Level 3 - should lint: {} {} {}", x, y, z);
            }
        }
    }
}

fn test_mixed_nesting() {
    let value = 42;
    let option = Some(10);
    let result: Result<i32, &str> = Ok(5);

    // Mixed types - Level 3 - Should trigger lint
    if value > 0 {
        if let Some(x) = option {
            if let Ok(y) = result {
                println!("Mixed level 3 - should lint: {} {} {}", value, x, y);
            }
        }
    }
}

fn test_early_return_good() {
    let value = 42;
    let option = Some(10);
    let result: Result<i32, &str> = Ok(5);

    // Good pattern - early returns instead of nesting
    if value <= 0 {
        return;
    }

    let Some(x) = option else {
        return;
    };

    let Ok(y) = result else {
        return;
    };

    println!("Early return pattern: {} {} {}", value, x, y);
}

fn main() {
    test_if_else_nesting();
    test_if_let_nesting();
    test_result_nesting();
    test_mixed_nesting();
    test_early_return_good();
}
