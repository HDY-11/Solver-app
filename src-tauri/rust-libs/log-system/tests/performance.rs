use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use log::LevelFilter;
use log_system::init_logging;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("log-system-performance");
    fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

fn parse_timestamp_ms(line: &str) -> Option<u64> {
    let date_time = line.split(" [").next()?;
    let parts: Vec<&str> = date_time.split(' ').collect();
    let time_part = parts.get(1)?;
    let segments: Vec<&str> = time_part.split(':').collect();
    let hour = segments.first()?.parse::<u64>().ok()?;
    let minute = segments.get(1)?.parse::<u64>().ok()?;
    let sec_ms = segments.get(2)?;
    let sec_parts: Vec<&str> = sec_ms.split('.').collect();
    let second = sec_parts.first()?.parse::<u64>().ok()?;
    let millis = sec_parts.get(1)?.parse::<u64>().ok()?;
    Some(hour * 3_600_000 + minute * 60_000 + second * 1000 + millis)
}

#[test]
fn all_performance_tests() {
    let dir = temp_dir();
    let high_path = dir.join("perf_high.log");
    let low_path = dir.join("perf_low.log");

    let (mut log_ctrl, log_handle) =
        init_logging(high_path.clone(), low_path.clone(), 65536).expect("初始化失败");
    log::set_max_level(LevelFilter::Info);

    self_referential_throughput(&log_handle, &low_path);
    self_referential_latency_distribution(&log_handle, &low_path);
    self_referential_flush_latency(&log_handle);

    log_ctrl.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

fn self_referential_throughput(log_handle: &log_system::LogHandle, low_path: &PathBuf) {
    const N: usize = 10_000;

    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    log::info!("__BENCHMARK_START__");
    for i in 0..N {
        log::info!("性能测试-{}", i);
    }
    log::info!("__BENCHMARK_END__");

    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let content = fs::read_to_string(low_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    let start_line = lines.iter().find(|l| l.contains("__BENCHMARK_START__"));
    let end_line = lines.iter().find(|l| l.contains("__BENCHMARK_END__"));

    if let (Some(start), Some(end)) = (start_line, end_line) {
        let start_ms = parse_timestamp_ms(start).unwrap_or(0);
        let end_ms = parse_timestamp_ms(end).unwrap_or(0);
        let elapsed = (end_ms.saturating_sub(start_ms)) as f64 / 1000.0;
        let throughput = N as f64 / elapsed.max(0.001);

        eprintln!("=== 吞吐量自指测试 ===");
        eprintln!("日志条目: {}", N);
        eprintln!("耗时: {:.3} 秒", elapsed);
        eprintln!("吞吐量: {:.0} 条/秒", throughput);
        eprintln!("每条日志: {:.1} 微秒", elapsed * 1_000_000.0 / N as f64);

        assert!(throughput > 1000.0, "吞吐量过低: {:.0}", throughput);
    }
}

fn self_referential_latency_distribution(log_handle: &log_system::LogHandle, low_path: &PathBuf) {
    const N: usize = 5_000;

    log_handle.clear().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    for i in 0..N {
        log::info!("LATENCY:{}", i);
    }

    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let content = fs::read_to_string(low_path).unwrap();
    let timestamps: Vec<u64> = content
        .lines()
        .filter(|l| l.contains("LATENCY:"))
        .filter_map(parse_timestamp_ms)
        .collect();

    if timestamps.len() < 2 {
        eprintln!("样本不足，跳过延迟测试");
        return;
    }

    let mut latencies: Vec<u64> = vec![];
    for i in 1..timestamps.len() {
        latencies.push(timestamps[i].saturating_sub(timestamps[i - 1]));
    }

    latencies.sort_unstable();
    let n = latencies.len();
    let avg = latencies.iter().sum::<u64>() / n as u64;
    let p50 = latencies[n * 50 / 100];
    let p95 = latencies[n * 95 / 100];
    let p99 = latencies[n * 99 / 100];

    eprintln!("=== 延迟分布自指测试 ===");
    eprintln!("样本数: {}", n);
    eprintln!("平均: {} ms", avg);
    eprintln!("P50:  {} ms", p50);
    eprintln!("P95:  {} ms", p95);
    eprintln!("P99:  {} ms", p99);

    assert!(p99 < 100, "P99 延迟过高: {} ms", p99);
}

fn self_referential_flush_latency(log_handle: &log_system::LogHandle) {
    log_handle.clear().unwrap();
    log_handle.emergency_flush_low().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    for i in 0..1000 {
        log::info!("刷盘测试-{}", i);
    }

    let start = Instant::now();
    log_handle.emergency_flush_low().unwrap();
    let elapsed = start.elapsed();

    eprintln!("=== 刷盘延迟自指测试 ===");
    eprintln!("耗时: {:?}", elapsed);
    assert!(elapsed.as_millis() < 1000, "刷盘超时: {:?}", elapsed);
}
