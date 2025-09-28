#![allow(unused, clippy::collapsible_if, clippy::single_match)]
use std::sync::LazyLock;

static LAZY_VALUE: LazyLock<i32> = LazyLock::new(|| {
    if let Some(config) = Some(42) {
        if config > 0 {
            if let Ok(validated) = Ok::<i32, &str>(config) {
                //~v ERROR: 4 levels
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
    let standalone_closure = || {
        if let Some(data) = Some(15) {
            if data > 10 {
                //~v ERROR: 5 levels
                if data < 50 {
                    if data % 5 == 0 {
                        println!("Closure nesting level 4!");
                    }
                }
            }
        }
    };
    standalone_closure();

    let outer_closure = || {
        let inner_closure = || {
            if let Ok(status) = Ok::<bool, &str>(true) {
                //~v ERROR: 4 levels
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
            _ => {
                if tag_id > 5 {
                    let _ = String::new();
                    //~v ERROR: 5 levels
                    if tag_id < 15 {
                        let _ = String::new();
                        if tag_id % 2 == 0 {
                            let _ = String::new();
                        }
                    }
                }
            }
        }
    }

    let tag_id = 5;
    let string = match tag_id {
        10 => String::new(),
        18 => String::new(),
        _ => {
            if tag_id > 5 {
                let _ = String::new();
                if tag_id < 15 {
                    let _ = String::new();
                    //~v ERROR: 4 levels
                    if tag_id % 2 == 0 {
                        let _ = String::new();
                    }
                }
            }
            String::new()
        }
    };

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
        if x < 2 {
            println!("x < 2");
            if x < 5 {
                println!("x < 5");
                //~v ERROR: 5 levels
                if x < 10 {
                    println!("x < 10");
                    if x < 20 {
                        println!("x < 20");
                    } else if x < 30 {
                        println!("x < 30");
                    } else if x < 40 {
                        println!("x < 40");
                    //~v ERROR: 5 found
                    } else if x < 50 {
                        println!("x < 50");
                    } else {
                        println!("x >= 50");
                    }
                }
            }
        } else if x < 3 {
            println!("x < 3");
        }
    } else if x < 2 {
        println!("x < 2");
    } else if x < 3 {
        println!("x < 3");
    //~v ERROR: 11 found
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
