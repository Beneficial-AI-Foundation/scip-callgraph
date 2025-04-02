// src/main.rs
mod helper;

fn main() {
    println!("Hello, world!");
    
    // Call a function from our module
    helper::utils::print_message("This is a test message");
    
    // Call a function with multiple dependencies
    let result = calculate_value(5, 10);
    println!("Calculated value: {}", result);
    
    // Call a function that calls other functions
    process_data();
}

fn calculate_value(a: i32, b: i32) -> i32 {
    let sum = a + b;
    let product = a * b;
    
    // Call a helper function
    helper::math::multiply(sum, 2) + helper::math::square(product)
}

fn process_data() {
    let data = vec![1, 2, 3, 4, 5];
    
    // Multiple function calls to create dependencies
    let processed = helper::data::transform_data(&data);
    let result = helper::data::analyze_data(&processed);
    
    println!("Processing result: {}", result);
}
