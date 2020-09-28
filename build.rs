#[cfg(target_os = "windows")]
fn main() {
    use std::io::Write;
    if std::env::var("PROFILE").unwrap() == "release" {
        let res = winres::WindowsResource::new();
        match res.compile() {
            Err(e) => {
                write!(std::io::stderr(), "{}", e).unwrap();
                std::process::exit(1);
            }
            Ok(_) => {}
        }
    }
}

// nothing to do for other operating systems
#[cfg(not(target_os = "windows"))]
fn main() {
}