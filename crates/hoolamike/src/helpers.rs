pub fn human_readable_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "kB", "MB", "GB", "TB", "PB"];

    if bytes < 1024 {
        return format!("{} {}", bytes, UNITS[0]);
    }

    let exponent = (bytes as f64).log(1024.0).floor() as usize;
    let exponent = exponent.min(UNITS.len() - 1);
    let value = bytes as f64 / 1024f64.powi(exponent as i32);

    format!("{:.2} {}", value, UNITS[exponent])
}
