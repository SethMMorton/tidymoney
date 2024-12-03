use tidymoney::interpret_dollar_amount;

fn main() {
    println!("Hello, world!");
    // store_old_transactions(Path::new("location"));
    println!("{}", interpret_dollar_amount("4.56", false));
}
