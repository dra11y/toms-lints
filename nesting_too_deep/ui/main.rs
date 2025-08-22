#![allow(unused)]
use std::sync::LazyLock;

//~v ERROR: 4 levels
static LAZY_VALUE: LazyLock<i32> = LazyLock::new(|| {
    if let Some(config) = Some(42) {
        if config > 0 {
            if let Ok(validated) = Ok::<i32, &str>(config) {
                if validated == 42 { 42 } else { 0 }
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    }
});

fn one() {
    let score = 42;
    let score_5_levels = 42;
    let maybe_count = Some(10);
    let maybe_count_5_levels = Some(10);
    let operation: Result<i32, usize> = Ok(5);

    if score != 0 {
        println!("Level 1 - OK");
    }

    if let Some(cnt) = maybe_count {
        if cnt < 100 {
            println!("Level 2 - OK");
        }
    }

    //~v ERROR: 6 levels
    if let Ok(op_result) = operation {
        if op_result < 100 {
            if score % 2 == 0 {
                println!("Level 3 - should lint");
                if op_result > 0 {
                    if score == 42 {
                        if op_result > 5 {
                            println!("Level 6 - should show 6 levels deep!");
                        }
                    }
                }
            }
        }
    }

    if score >= 1 {
        println!("Level 1 - OK");
    }

    //~v ERROR: 4 levels
    if let Some(count_val) = maybe_count {
        if count_val > 5 {
            if score % 3 == 0 {
                if score != 99 {
                    println!("Level 4 - should show 4 levels deep, not 3!");
                }
            }
        }
    }

    if let Ok(res) = operation {
        if res < 100 {
            println!("Level 2 - OK");
        }
    }

    if score > 0 {
        println!("Level 1 - OK");
    }

    //~v ERROR: 5 levels
    if let Some(x) = maybe_count {
        if let Ok(y) = operation {
            if x > 5 {
                if y < 50 {
                    if score == 42 {
                        println!("Level 5 - should show 5 levels deep!");
                    }
                }
            }
        }
    }

    //~v ERROR: 6 levels
    loop {
        for i in 0..10 {
            while i > 0 {
                loop {
                    if true {
                        if true {
                            println!("Loop nesting level 6!");
                        }
                    }
                    break;
                }
                break;
            }
        }
        break;
    }

    match score {
        42 => match maybe_count {
            Some(x) => match operation {
                Ok(y) => println!("Match nesting level 3: {} {}", x, y),
                Err(_) => {}
            },
            None => {}
        },
        _ => {}
    }

    //~v ERROR: 5 levels
    let closure_5_levels = || {
        if score_5_levels != 0 {
            let inner_closure_5_levels = || {
                if let Some(val) = maybe_count_5_levels {
                    if val < 100 {
                        println!("Closure nesting level 5!");
                    }
                }
            };
            inner_closure_5_levels();
        }
    };
    closure_5_levels();

    //~v ERROR: 4 levels
    if let Ok(num) = operation {
        loop {
            match maybe_count {
                Some(x) => {
                    let closure_4_levels = || {
                        if true {
                            println!("Mixed nesting level 4: {}", x);
                        }
                    };
                    closure_4_levels();
                    break;
                }
                None => break,
            }
        }
    }
}

fn two() {
    // TODO ~v ERROR: 5 levels
    let standalone_closure = || {
        if let Some(data) = Some(15) {
            if data > 10 {
                if data < 50 {
                    if data % 5 == 0 {
                        println!("Closure nesting level 4!");
                    }
                }
            }
        }
    };
    standalone_closure();

    //~v ERROR: 4 levels
    let outer_closure = || {
        let inner_closure = || {
            if let Ok(status) = Ok::<bool, &str>(true) {
                if status {
                    println!("Nested closure with if nesting level 4!");
                }
            }
        };
        inner_closure();
    };
    outer_closure();
}

fn three() {
    let response: Option<Result<Vec<i32>, &str>> = Some(Ok(vec![1, 2, 3]));
    let has_permission = true;
    let is_authenticated = false;

    // TODO ~v ERROR: 4 levels
    if let Some(result) = response {
        match result {
            Ok(items) => {
                if has_permission {
                    println!("Has permission: {:?}", items);
                } else if is_authenticated {
                    panic!("Unexpected auth response");
                } else {
                    panic!("Unexpected response type");
                }
            }
            Err(e) => panic!("Error: {:?}", e),
        }
    } else {
        panic!("No response received");
    }

    //~v ERROR: 5 levels
    if let Some(token) = Some(42) {
        if token != 0 {
            match token {
                42 => {
                    if let Ok(validated) = Ok::<bool, &str>(true) {
                        if validated {
                            println!("First condition");
                        }
                    } else if let Err(msg) = Err::<bool, &str>("test") {
                        if msg.len() > 0 {
                            println!("Second condition");
                        }
                    } else {
                        println!("Final condition");
                    }
                }
                _ => {}
            }
        }
    }
}

fn four() {
    while let Some(tag_id) = Some(10) {
        match tag_id {
            10 => {
                let _ = String::new();
            }
            18 => {
                let _ = 20;
            }
            _ => {}
        }
    }

    while let Some(message_tag) = Some(10) {
        match message_tag {
            10 => {
                if let Ok(field) = Ok::<String, &str>(String::new()) {
                    let _ = field;
                }
            }
            18 => {
                if message_tag > 5 {
                    let _ = String::new();
                }
            }
            _ => {}
        }
    }
}

fn five() {
    while let Some(identifier) = Some(10) {
        match identifier {
            10 => {
                let _ = String::new();
            }
            18 => {
                let _ = String::new();
            }
            _ => {}
        }
    }
}

fn six() {
    while let Some(packet_id) = Some(10) {
        match packet_id {
            10 => {
                if packet_id == 10 {
                    let _ = String::new();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    // Force the LazyLock to initialize to test the nesting
    let _value = *LAZY_VALUE;

    one();
    two();
    three();
    four();
    five();
    six();
}
