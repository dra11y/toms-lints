#![allow(unused)]
use std::sync::LazyLock;

static LAZY_VALUE: LazyLock<i32> = LazyLock::new(|| {
    //~v ERROR: 4 levels
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

fn two() {
    //~v ERROR: 5 levels
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

// should not lint
// TODO: check for too many else ifs -- we need a more complex linter for control flow
fn seven() {
    let x = 1;
    if x < 1 {
        println!("x < 1");
    } else if x < 2 {
        println!("x < 2");
    } else if x < 3 {
        println!("x < 3");
    } else if x < 4 {
        println!("x < 4");
    } else if x < 5 {
        println!("x < 5");
    } else if x < 6 {
        println!("x < 6");
    } else if x < 7 {
        println!("x < 7");
    } else if x < 8 {
        println!("x < 8");
    } else if x < 9 {
        println!("x < 9");
    } else if x < 10 {
        println!("x < 10");
    } else {
        println!("x >= 10");
    }
}

fn main() {
    // Force the LazyLock to initialize to test the nesting
    let _value = *LAZY_VALUE;

    two();
    four();
    five();
    six();
    seven();
}
