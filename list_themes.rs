use syntect::highlighting::ThemeSet;

fn main() {
    let theme_set = ThemeSet::load_defaults();
    println!("Available syntect themes:");
    for (name, _theme) in &theme_set.themes {
        println!("  - {}", name);
    }
}
