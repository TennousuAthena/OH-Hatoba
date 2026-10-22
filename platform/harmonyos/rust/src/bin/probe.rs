fn main() {
    let report = hatoba_ohos::probe();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if !report.passed() {
        std::process::exit(1);
    }
}
