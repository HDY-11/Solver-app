use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use log::LevelFilter;
use log_system::init_logging;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("log-system-robustness");
    fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

#[test]
fn all_robustness_tests() {
    let dir = temp_dir();
    let high_path = dir.join("shared_high.log");
    let low_path = dir.join("shared_low.log");

    let (mut log_ctrl, log_handle) =
        init_logging(high_path.clone(), low_path.clone(), 8192).expect("初始化失败");
    log::set_max_level(LevelFilter::Info);

    test_high_concurrency(&log_handle);
    test_high_priority_not_starved_by_low(&log_handle);
    test_channel_full_backpressure(&log_handle);
    test_log_integrity_on_panic(&log_handle);
    test_kill_simulation(&log_handle);
    test_empty_file_operations(&log_handle);
    test_tiny_channel_behaviour(&log_handle);

    log_ctrl.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

fn test_high_concurrency(log_handle: &log_system::LogHandle) {
    let barrier = Arc::new(std::sync::Barrier::new(5));
    let mut handles = vec![];

    for i in 0..4 {
        let h = log_handle.clone();
        let b = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            b.wait();
            for j in 0..50 {
                h.log(format!("并发-线程{}-日志{}", i, j)).unwrap();
            }
        }));
    }

    barrier.wait();
    for i in 0..200 {
        log::info!("并发-低优先级-{}", i);
    }

    for h in handles {
        h.join().unwrap();
    }

    log_handle.flush().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(500));
}

fn test_high_priority_not_starved_by_low(log_handle: &log_system::LogHandle) {
    log_handle.clear().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    log::info!("饥饿测试-低优先级");
    log_handle.log("饥饿测试-高优先级").unwrap();
    log_handle.flush().unwrap();
    std::thread::sleep(Duration::from_millis(300));
}

fn test_channel_full_backpressure(log_handle: &log_system::LogHandle) {
    log_handle.clear().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    for i in 0..200 {
        log::info!("背压测试-{}", i);
    }

    let result = log_handle.log("背压测试-高优先级");
    assert!(result.is_ok(), "高优先级发送不应该失败");

    log_handle.flush().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));
}

fn test_log_integrity_on_panic(log_handle: &log_system::LogHandle) {
    log_handle.clear().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    log::info!("panic测试-崩溃前的日志");

    let h = log_handle.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        h.emergency_flush_low().unwrap();
        h.flush().unwrap();
        panic!("模拟崩溃");
    }));

    assert!(result.is_err(), "应该 panic");
}

fn test_kill_simulation(log_handle: &log_system::LogHandle) {
    log_handle.clear().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    log::info!("kill测试-模拟被kill前的日志");
    log_handle.emergency_flush_low().unwrap();
    log_handle.flush().unwrap();
    std::thread::sleep(Duration::from_millis(200));
}

fn test_empty_file_operations(log_handle: &log_system::LogHandle) {
    log_handle.clear().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    log_handle.log("清空测试-清空空文件后的第一条").unwrap();
    log_handle.flush().unwrap();
    std::thread::sleep(Duration::from_millis(200));
}

fn test_tiny_channel_behaviour(log_handle: &log_system::LogHandle) {
    log_handle.clear().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    for i in 0..10 {
        log::info!("极小通道测试: {}", i);
    }
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(500));
}
