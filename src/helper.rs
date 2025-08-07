// src/helper.rs
pub mod utils {
    pub fn print_message(msg: &str) {
        println!("Message: {msg}");
    }

    pub fn format_string(s: &str) -> String {
        format!("Formatted: {}", s.to_uppercase())
    }
}

pub mod math {
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    pub fn multiply(a: i32, b: i32) -> i32 {
        a * b
    }

    pub fn square(a: i32) -> i32 {
        a * a
    }
}

pub mod data {
    pub fn transform_data(data: &[i32]) -> Vec<i32> {
        data.iter().map(|x| x * 2).collect()
    }

    pub fn analyze_data(data: &[i32]) -> i32 {
        data.iter().sum()
    }

    pub fn filter_data(data: &[i32], threshold: i32) -> Vec<i32> {
        data.iter().filter(|&&x| x > threshold).cloned().collect()
    }
}
