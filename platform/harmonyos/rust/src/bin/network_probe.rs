#[tokio::main]
async fn main() {
    let area = std::env::temp_dir().join(format!("oh-hatoba-network-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&area).unwrap();
    let report = hatoba_ohos::network_probe::run(&area).await;
    std::fs::remove_dir_all(area).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if !report["checks"]
        .as_object()
        .is_some_and(|checks| checks.len() >= 9 && checks.values().all(|v| v == true))
    {
        std::process::exit(1);
    }
}
