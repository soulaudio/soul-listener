//! Serde Serialization Example
//!
//! Demonstrates serializing and deserializing display specs to/from JSON.
//!
//! Run with: cargo run --example serde_example --features serde

#[cfg(feature = "serde")]
fn main() {
    use eink_specs::displays::WAVESHARE_2_13_V4;

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&WAVESHARE_2_13_V4).unwrap();
    println!("Serialized DisplaySpec:");
    println!("{}", json);
    println!();

    // Note: Deserialization from JSON requires handling the 'static lifetime for the name field.
    // In a real application, you would typically deserialize into a struct with owned strings,
    // or use a custom deserializer.

    println!("The DisplaySpec can be serialized to JSON for configuration files.");
    println!("For deserialization, consider using a separate config struct with owned Strings.");
}

#[cfg(not(feature = "serde"))]
fn main() {
    println!("This example requires the 'serde' feature.");
    println!("Run with: cargo run --example serde_example --features serde");
}
