use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use log::LevelFilter;
use log_system::{LogHandle, init_logging};

fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("log-system-diag");
    fs::create_dir_all(&dir).expect("无法创建临时目录");
    dir.join(name)
}

#[test]
fn diagnostic_test() {
    let high_path = temp_path("diag_high.log");
    let low_path = temp_path("diag_low.log");

    eprintln!("=== 1. 初始化日志系统 ===");
    let (log_ctrl, log_handle) =
        init_logging(high_path.clone(), low_path.clone(), 4096).expect("初始化失败");
    log::set_max_level(LevelFilter::Info);

    // 等待后台线程完全启动
    std::thread::sleep(Duration::from_millis(200));

    eprintln!("=== 2. 清空高优先级文件（初始为空） ===");
    log_handle.clear().expect("发送 Clear 命令失败");
    std::thread::sleep(Duration::from_millis(300)); // 确保清空操作完成

    // 检查高优先级文件是否为空
    let meta = fs::metadata(&high_path).expect("无法获取高优先级文件元数据");
    eprintln!("清空后高优先级文件大小: {} 字节", meta.len());
    assert_eq!(meta.len(), 0, "清空后文件大小应为 0");

    eprintln!("=== 3. 写入两条高优先级日志 ===");
    log_handle
        .log("诊断日志：清空后的第一条")
        .expect("发送日志失败");
    log::info!("低优先级写入,查看是否是select阻塞了高优先级日志");
    log_handle
        .log("诊断日志：清空后的第二条")
        .expect("发送日志失败");

    eprintln!("=== 4. 手动刷新高优先级 ===");
    log_handle.flush().expect("刷新日志失败");
    std::thread::sleep(Duration::from_millis(300));

    // 读取高优先级文件并打印原始内容
    let mut high_content = String::new();
    let mut file = File::open(&high_path).expect("无法打开高优先级文件");
    file.read_to_string(&mut high_content)
        .expect("读取高优先级文件失败");
    eprintln!("高优先级文件长度: {} 字节", high_content.len());
    eprintln!("高优先级文件内容(转义显示): {:?}", high_content);

    assert!(
        high_content.contains("诊断日志：清空后的第一条"),
        "高优先级文件缺少预期的日志内容。\n实际内容: {:?}",
        high_content
    );

    eprintln!("=== 5. 测试低优先级日志 ===");
    log::info!("低优先级：应用启动");
    log::warn!("低优先级：警告");
    log::error!("低优先级：错误");

    // 紧急排空低优先级
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let mut low_content = String::new();
    fs::File::open(&low_path)
        .unwrap()
        .read_to_string(&mut low_content)
        .unwrap();
    eprintln!("低优先级文件内容(转义显示): {:?}", low_content);
    assert!(low_content.contains("低优先级：应用启动"));
    assert!(low_content.contains("低优先级：警告"));
    assert!(low_content.contains("低优先级：错误"));

    eprintln!("=== 6. 再次清空高优先级并验证 ===");
    log_handle.clear().unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let meta = fs::metadata(&high_path).unwrap();
    eprintln!("第二次清空后文件大小: {} 字节", meta.len());
    assert_eq!(meta.len(), 0, "第二次清空后文件大小应为 0");

    // 写入新日志
    log_handle.log("第二次清空后日志").unwrap();
    log_handle.flush().unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let mut content2 = String::new();
    File::open(&high_path)
        .unwrap()
        .read_to_string(&mut content2)
        .unwrap();
    eprintln!("第二次写入后内容: {:?}", content2);
    assert!(content2.contains("第二次清空后日志"));

    // 正常关闭
    eprintln!("=== 7. 关闭系统 ===");

    // 清理
    let _ = fs::remove_file(&high_path);
    let _ = fs::remove_file(&low_path);
}
