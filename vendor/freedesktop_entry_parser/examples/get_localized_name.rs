use freedesktop_entry_parser::parse_entry;

fn main() -> std::io::Result<()> {
    let lang = std::env::args().nth(1).expect("Not enough args");
    let entry = parse_entry("./test_data/firefox.desktop")?;
    let desktop_section = entry.section("Desktop Entry").unwrap();
    match desktop_section.attr_with_param("GenericName", lang).get(0) {
        Some(localized_name) => println!("{localized_name}"),
        None => println!("No name for that lang"),
    }
    Ok(())
}
