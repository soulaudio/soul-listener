use embedded_graphics::pixelcolor::Gray4;

fn main() {
    println!("BLACK: {:?}", Gray4::BLACK);
    println!("WHITE: {:?}", Gray4::WHITE);
    println!("BLACK luma: {}", Gray4::BLACK.luma());
    println!("WHITE luma: {}", Gray4::WHITE.luma());
}
