#[cfg(not(target_os = "android"))]
fn main() {
    opensus::run();
}

#[cfg(target_os = "android")]
fn main() {}
