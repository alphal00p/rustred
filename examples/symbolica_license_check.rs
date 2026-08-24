use symbolica::LicenseManager;

fn main() {
    if LicenseManager::is_licensed() {
        println!("Symbolica license is active");
    } else {
        eprintln!("Symbolica license is not active");
        std::process::exit(1);
    }
}
