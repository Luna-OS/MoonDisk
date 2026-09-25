//! The privileged helper the native macOS app starts through an
//! administrator prompt: `moondisk-helper --listen <socket> --owner <uid>`.
//! See `moondisk_lib::helper`.

fn main() {
    #[cfg(unix)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Err(e) = moondisk_lib::helper::run_from_args(&args) {
            eprintln!("moondisk-helper: {e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(unix))]
    {
        eprintln!("moondisk-helper only runs on macOS and Linux");
        std::process::exit(1);
    }
}
