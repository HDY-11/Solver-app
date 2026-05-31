//! 批量数据接口：将粒子状态导出为连续数组，或从数组设置粒子状态。
//!
//! 这些函数避免在 Python 逐粒子调用中产生大量 FFI 开销。

use nalgebra::{Point3, Vector3};
use crate::coordinate_system::CoordinateSystem3;

/// 将所有粒子的位置导出为 N×3 矩阵（按行存储 x,y,z）
pub fn positions_matrix(system: &CoordinateSystem3) -> Vec<f64> {
    let n = system.particle_count();
    let mut data = Vec::with_capacity(n * 3);
    for p in system.particles() {
        data.push(p.position.x);
        data.push(p.position.y);
        data.push(p.position.z);
    }
    data
}

/// 将所有粒子的速度导出为 N×3 矩阵
pub fn velocities_matrix(system: &CoordinateSystem3) -> Vec<f64> {
    let n = system.particle_count();
    let mut data = Vec::with_capacity(n * 3);
    for p in system.particles() {
        data.push(p.v.x);
        data.push(p.v.y);
        data.push(p.v.z);
    }
    data
}

/// 将所有粒子的加速度导出为 N×3 矩阵
pub fn accelerations_matrix(system: &CoordinateSystem3) -> Vec<f64> {
    let n = system.particle_count();
    let mut data = Vec::with_capacity(n * 3);
    for p in system.particles() {
        data.push(p.a.x);
        data.push(p.a.y);
        data.push(p.a.z);
    }
    data
}

/// 将所有粒子的质量导出为长度 N 的向量
pub fn masses_vector(system: &CoordinateSystem3) -> Vec<f64> {
    system.particles().iter().map(|p| p.m).collect()
}

/// 从 N×9 数组设置所有粒子的完整状态 [pos_x, pos_y, pos_z, vel_x, vel_y, vel_z, acc_x, acc_y, acc_z]
pub fn set_full_state(system: &mut CoordinateSystem3, data: &[f64]) {
    let n = system.particle_count();
    assert_eq!(data.len(), n * 9);
    for (i, p) in system.particles_mut().iter_mut().enumerate() {
        let base = i * 9;
        p.position = Point3::new(data[base], data[base + 1], data[base + 2]);
        p.v = Vector3::new(data[base + 3], data[base + 4], data[base + 5]);
        p.a = Vector3::new(data[base + 6], data[base + 7], data[base + 8]);
    }
}

/// 将形状采样点导出为 N×3 数组
pub fn points_to_matrix(points: &[Point3<f64>]) -> Vec<f64> {
    let mut data = Vec::with_capacity(points.len() * 3);
    for p in points {
        data.push(p.x);
        data.push(p.y);
        data.push(p.z);
    }
    data
}

/// 计算遮蔽率：对于每个采样点，判断其是否在 p1-p2 线段距离 d 以内。
/// 返回被遮蔽的点的比例。
pub fn occlusion_ratio(
    m1: &Point3<f64>,
    ymd: &Point3<f64>,
    sample_points: &[Point3<f64>],
    threshold: f64,
) -> f64 {
    if sample_points.is_empty() {
        return 0.0;
    }
    let mut count = 0usize;
    for s in sample_points {
        if super::coordinate_system::is_within_distance(m1, s, ymd, threshold) {
            count += 1;
        }
    }
    count as f64 / sample_points.len() as f64
}