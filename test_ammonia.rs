use ammonia::Builder;

fn main() {
    let html = r#"<img src="test.jpg" alt="foo < bar">"#;
    let mut builder = Builder::new();
    builder.add_tags(&["img"]);
    builder.add_generic_attributes(&["src", "alt"]);
    let cleaned = builder.clean(html).to_string();
    println!("Original: {}", html);
    println!("Cleaned:  {}", cleaned);

    let html2 = r#"<img src="test.jpg" alt="foo &lt; bar">"#;
    let cleaned2 = builder.clean(html2).to_string();
    println!("Original 2: {}", html2);
    println!("Cleaned 2:  {}", cleaned2);
}
