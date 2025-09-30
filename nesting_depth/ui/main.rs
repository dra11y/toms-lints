#![allow(unused, clippy::collapsible_if, clippy::single_match)]
use std::sync::LazyLock;

static LAZY_VALUE: LazyLock<i32> = LazyLock::new(|| {
    if let Some(config) = Some(42) {
        if config > 0 {
            // ctxs if: 41, else: 69
            if let Ok(validated) = Ok::<i32, &str>(config) {
                //~v ERROR: 4 levels
                if validated == 42 { 42 } else { 0 }
            // ctxs if: 70, else-if: 87
            } else if config < 1 {
                //~v ERROR: 4 levels
                {
                    let x = 1;
                    x + 1
                }
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
                //~v ERROR: 5 levels
                if tag_id < 15 {
                    let _ = String::new();
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
    fn eight() {
        let y = 1;
        if y < 1 {
            println!("y < 1");
            if y < 2 {
                println!("y < 2");
                //~v ERROR: 6 levels
                if y < 5 {
                    println!("y < 5");
                    if y < 10 {
                        println!("y < 10");
                        if y < 20 {
                            println!("y < 20");
                        } else if y < 30 {
                            println!("y < 30");
                        } else if y < 40 {
                            println!("y < 40");
                        } else if y < 50 {
                            println!("y < 50");
                        } else {
                            println!("y >= 50");
                        }
                    }
                }
            } else if y < 3 {
                println!("y < 3");
            }
        }
    }
    let x = 1;
    //~v ERROR: 22 found
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
                    } else if x < 50 {
                        println!("x < 50");
                    } else if x < 60 {
                        println!("x < 60");
                    } else if x < 70 {
                        println!("x < 70");
                    } else {
                        println!("x >= 70");
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
    // Additional edge cases
    edge_let_else_reduction();
    edge_deep_only_in_final_else(5);
    edge_mixed_match_loop_closure(true, 0);
    edge_sequential_independent_ifs(10);
    edge_nested_matches(0);
    edge_macro_local(9);
    edge_multiple_closures_layers(4);
    edge_partial_deep_path(true);
}

// --- Edge case functions for additional lint coverage ---

#[allow(unused)]
fn edge_let_else_reduction() {
    let some_val = Some(10);
    let Some(a) = some_val else {
        return;
    };
    let inner = Some(a + 2);
    let Some(b) = inner else {
        return;
    };
    if b > 5 {
        if b % 2 == 0 {
            if b < 20 {
                //~v ERROR: 4 levels
                if b != 13 {
                    let _ = b; // silence copy drop
                }
            }
        }
    }
}

#[allow(unused)]
fn edge_deep_only_in_final_else(x: i32) {
    if x < 0 {
        if x < -5 {
            let _ = x;
        }
    } else if x == 0 {
        if x + 1 == 1 {
            let _ = x;
        }
    } else {
        if x > 1 {
            if x > 2 {
                //~v ERROR: 5 levels
                if x > 3 {
                    if x > 4 {
                        let _ = x;
                    }
                }
            }
        }
    }
}

#[allow(unused)]
fn edge_mixed_match_loop_closure(flag: bool, code: u32) {
    let mut i = 0;
    while i < 2 {
        match code {
            0 => {
                let f = || {
                    //~v ERROR: 7 levels
                    if flag {
                        if code == 0 {
                            if i == 0 {
                                if i < 10 {
                                    let _ = i;
                                }
                            }
                        }
                    }
                };
                f();
            }
            1 => {
                if flag {
                    let _ = code;
                }
            }
            _ => {}
        }
        i += 1;
    }
}

#[allow(unused)]
fn edge_sequential_independent_ifs(n: i32) {
    if n > 0 {
        let _ = n;
    }
    if n > 1 {
        let _ = n;
    }
    if n > 2 {
        let _ = n;
    }
    if n > 3 {
        let _ = n;
    }
    if n > 4 {
        let _ = n;
    }
}

#[allow(unused)]
fn edge_nested_matches(v: i32) {
    match v {
        0 => match v + 1 {
            1 => match v + 2 {
                //~v ERROR: 6 levels
                2 => {
                    if v == 0 {
                        if v + 3 == 3 {
                            let _ = v;
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        },
        _ => {}
    }
}

macro_rules! make_nested_if {
    ($val:expr) => {
        if $val > 0 {
            if $val > 1 {
                if $val > 2 {
                    //~v ERROR: 5 levels
                    if $val > 3 {
                        if $val > 4 {
                            let _ = $val;
                        }
                    }
                }
            }
        }
    };
}

#[allow(unused)]
fn edge_macro_local(v: i32) {
    make_nested_if!(v);
}

#[allow(unused)]
fn edge_multiple_closures_layers(x: i32) {
    let outer = || {
        let mid = || {
            let inner = || {
                //~v ERROR: 7 levels
                if x > 0 {
                    if x > 1 {
                        if x > 2 {
                            if x > 3 {
                                let _ = x;
                            }
                        }
                    }
                }
            };
            inner();
        };
        mid();
    };
    outer();
}

#[allow(unused)]
fn edge_partial_deep_path(cond: bool) {
    if cond {
        if cond {
            if cond {
                //~v ERROR: 4 levels
                if cond {
                    let _ = cond;
                }
            }
        }
    } else if !cond {
        // shallow alternative branch
        let _ = cond;
    }
}
