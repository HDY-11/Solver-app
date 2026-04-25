use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Duration;

use log::LevelFilter;
use log_system::init_logging;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("log-system-deep-panic");
    fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 深层 panic 测试：直接使用 panic 钩子验证抢救能力。
///
/// 评价指标：panic 发生后，低优先级日志文件中实际写入的日志条目数量。
#[test]
fn deep_panic_resilience_with_hook() {
    let dir = temp_dir();
    let high_path = dir.join("deep_panic_high.log");
    let low_path = dir.join("deep_panic_low.log");

    let (mut log_ctrl, log_handle) =
        init_logging(high_path.clone(), low_path.clone(), 65536).expect("初始化失败");
    log::set_max_level(LevelFilter::Info);

    // 安装 panic 钩子（与主程序完全一致）
    let panic_handle = log_handle.clone();
    std::panic::set_hook(Box::new(move |info| {
        let _ = panic_handle.emergency_flush_low();
        let _ = panic_handle.flush();
        eprintln!("Panic occurred: {}", info);
    }));

    let low_path_clone = low_path.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        // ========== 第一阶段：正常写入 ==========
        for i in 0..2000 {
            log::info!("PRE_PANIC_NORMAL_{}", i);
        }

        // 确保初始数据有足够时间被后台线程消费
        std::thread::sleep(Duration::from_millis(200));

        // ========== 第二阶段：写入关键数据 ==========
        const CRITICAL_COUNT: usize = 500;
        for i in 0..CRITICAL_COUNT {
            log::info!("CRITICAL_BEFORE_PANIC_{}", i);
        }

        // ========== 第三阶段：触发 panic（钩子自动抢救）==========
        panic!("模拟深层 panic");
    }));

    // 验证 panic 确实被捕获
    assert!(result.is_err(), "应该成功捕获 panic");

    // 给后台线程额外的时间完成钩子中的 emergency_flush_low
    std::thread::sleep(Duration::from_millis(500));

    // 正常关闭日志系统
    log_ctrl.shutdown();

    // ========== 评价指标 ==========
    let critical_count = count_lines_with_prefix(&low_path, "CRITICAL_BEFORE_PANIC_");
    let normal_count = count_lines_with_prefix(&low_path, "PRE_PANIC_NORMAL_");

    eprintln!("===== 深层 Panic 测试结果（使用钩子）=====");
    eprintln!("正常写入 (PRE_PANIC): {} / 2000 条", normal_count);
    eprintln!("关键数据 (CRITICAL): {} / 500 条", critical_count);
    eprintln!(
        "关键数据保存率: {:.1}%",
        critical_count as f64 / 500.0 * 100.0
    );

    // 断言：关键数据的保存率必须达到 95% 以上
    assert!(
        critical_count >= 475,
        "关键数据丢失过多: 期望 >= 475, 实际 {}",
        critical_count
    );

    // 正常写入的数据也应该大部分保存
    assert!(
        normal_count >= 1800,
        "正常数据丢失过多: 期望 >= 1800, 实际 {}",
        normal_count
    );

    // 清理
    let _ = fs::remove_dir_all(&dir);
}

/// 统计文件中包含指定前缀的行数
fn count_lines_with_prefix(path: &PathBuf, prefix: &str) -> usize {
    match File::open(path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            reader
                .lines()
                .filter_map(|line| line.ok())
                .filter(|line| line.contains(prefix))
                .count()
        }
        Err(_) => 0,
    }
}
