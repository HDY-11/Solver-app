//! 核心物理类型：粒子、几何形状、坐标系统。
//!
//! 本模块提供三维空间中的基本运动学模拟能力：
//! - [`Particle3`] 质点，带位置、速度、加速度、质量。
//! - [`Sharps3`] 几何形状枚举，支持表面采样。
//! - [`CoordinateSystem3`] 管理一组粒子，提供时间推进和全局更新。

use nalgebra::{Point3, Vector3};
use std::f64::consts::PI;

/// 粒子集合类型
pub type Particle3s = Vec<Particle3>;
/// 三维点集合类型
pub type Points<T> = Vec<Point3<T>>;

/// 重力加速度 (m/s²)
pub static G: f64 = 9.8;

// ---------------------------------------------------------------------------
// 粒子
// ---------------------------------------------------------------------------

/// 三维空间中的质点。
///
/// # 字段说明
/// - `id`：用户自定义标识符
/// - `position`：位置 (m)
/// - `m`：质量 (kg)，必须 ≥ 0
/// - `v`：速度 (m/s)
/// - `a`：固有加速度 (m/s²)，例如重力引起的分量
/// - `t`：该状态对应的最后更新时间 (s)
///
/// # 示例
/// ```
/// use nalgebra::{Point3, Vector3};
/// use simple_motion::coordinate_system::Particle3;
///
/// let p = Particle3::new(Point3::new(0.0, 0.0, 0.0), 1)
///     .with_velocity(Vector3::new(1.0, 0.0, 0.0))
///     .with_mass(2.0);
/// ```
#[derive(Debug, Clone)]
pub struct Particle3 {
    pub id: usize,
    /// 位置 (m)
    pub position: Point3<f64>,
    /// 质量 (kg)
    pub m: f64,
    /// 速度 (m/s)
    pub v: Vector3<f64>,
    /// 固有加速度 (m/s²)
    pub a: Vector3<f64>,
    /// 状态更新时间 (s)
    pub t: f64,
}

impl Particle3 {
    /// 创建新粒子，初始速度、加速度均为零。
    pub fn new(p: Point3<f64>, id: usize) -> Self {
        Self {
            id,
            position: p,
            m: 1.0,
            v: Vector3::zeros(),
            a: Vector3::zeros(),
            t: 0.0,
        }
    }

    /// 设置速度（Builder 模式）
    pub fn with_velocity(mut self, v: Vector3<f64>) -> Self {
        self.v = v;
        self
    }

    /// 设置固有加速度（Builder 模式）
    pub fn with_acceleration(mut self, a: Vector3<f64>) -> Self {
        self.a = a;
        self
    }

    /// 设置质量（Builder 模式）
    ///
    /// # Panics
    /// 如果质量 < 0
    pub fn with_mass(mut self, m: f64) -> Self {
        assert!(m >= 0.0, "质量必须为非负数");
        self.m = m;
        self
    }

    /// 设置最后更新时间（Builder 模式）
    pub fn with_lastest_time(mut self, t: f64) -> Self {
        self.t = t;
        self
    }

    /// 标准运动学更新：使用当前速度和加速度推进位置和速度。
    pub fn update(&mut self, dt: f64) {
        self.update_position(dt);
        self.update_velocity(dt);
        self.t += dt;
    }

    /// 仅更新位置
    pub fn update_position(&mut self, dt: f64) {
        self.position += self.v * dt + 0.5 * self.a * dt * dt;
    }

    /// 仅更新速度
    pub fn update_velocity(&mut self, dt: f64) {
        self.v += self.a * dt;
    }

    /// 使用指定加速度更新位置（不改变 self.a）
    pub fn update_position_with(&mut self, a: Vector3<f64>, dt: f64) {
        self.position += self.v * dt + 0.5 * a * dt * dt;
    }

    /// 使用指定加速度更新速度（不改变 self.a）
    pub fn update_velocity_with(&mut self, a: Vector3<f64>, dt: f64) {
        self.v += a * dt;
    }

    /// 施加外力，自动转换为加速度并累积到 self.a
    ///
    /// # Panics
    /// 如果质量为 0
    pub fn apply_force(&mut self, force: Vector3<f64>) {
        assert!(self.m > 0.0, "质量必须为正数才能施加力");
        self.a += force / self.m;
    }

    /// 使用外力推进（结合固有加速度）
    pub fn update_with_force(&mut self, force: Vector3<f64>, dt: f64) {
        assert!(self.m > 0.0, "质量必须为正数");
        let a = (force / self.m) + self.a;
        self.update_velocity_with(a, dt);
        self.update_position_with(a, dt);
        self.t += dt;
    }

    /// 使用附加加速度推进（不改变 self.a）
    pub fn update_with_acceleration(&mut self, a: Vector3<f64>, dt: f64) {
        let a = a + self.a;
        self.update_velocity_with(a, dt);
        self.update_position_with(a, dt);
        self.t += dt;
    }

    /// 重置固有加速度（通常在每帧开始前调用）
    pub fn reset_acceleration(&mut self) {
        self.a = Vector3::zeros();
    }

    /// 将粒子状态（位置和速度）打包成 6 元素数组：[x, y, z, vx, vy, vz]
    pub fn to_state_array(&self) -> [f64; 6] {
        [
            self.position.x,
            self.position.y,
            self.position.z,
            self.v.x,
            self.v.y,
            self.v.z,
        ]
    }

    /// 从 6 元素数组设置位置和速度
    pub fn set_from_state_array(&mut self, state: &[f64; 6]) {
        self.position = Point3::new(state[0], state[1], state[2]);
        self.v = Vector3::new(state[3], state[4], state[5]);
    }
}

// ---------------------------------------------------------------------------
// 几何形状与采样
// ---------------------------------------------------------------------------

/// 三维几何形状，可用于遮蔽分析、碰撞检测等。
///
/// 每种形状都包含一个 `sample_points` 方法，按指定密度生成表面离散点。
#[derive(Debug, Clone)]
pub enum Sharps3 {
    /// 圆柱表面（含上、下底面和侧面）
    CylinderSurface {
        /// 下底面中心
        center: Point3<f64>,
        /// 半径 (m)
        radius: f64,
        /// 高度 (m)
        height: f64,
        /// 采样密度 (points/m²)
        density: f64,
    },
    /// 球体表面
    SphereSurface {
        /// 球心
        center: Point3<f64>,
        /// 半径 (m)
        radius: f64,
        /// 采样密度 (points/m²)
        density: f64,
    },
    /// 矩形平面
    Plane {
        /// 平面上一点
        point: Point3<f64>,
        /// 法向量（自动归一化）
        normal: Vector3<f64>,
        /// 宽度 (m)
        width: f64,
        /// 高度 (m)
        height: f64,
        /// 采样密度 (points/m²)
        density: f64,
    },
}

impl Sharps3 {
    /// 创建圆柱表面
    pub fn cylinder_surface(center: Point3<f64>, radius: f64, height: f64, density: f64) -> Self {
        Sharps3::CylinderSurface {
            center,
            radius,
            height,
            density,
        }
    }

    /// 创建球体表面
    pub fn sphere_surface(center: Point3<f64>, radius: f64, density: f64) -> Self {
        Sharps3::SphereSurface {
            center,
            radius,
            density,
        }
    }

    /// 创建平面
    pub fn plane(
        point: Point3<f64>,
        normal: Vector3<f64>,
        width: f64,
        height: f64,
        density: f64,
    ) -> Self {
        Sharps3::Plane {
            point,
            normal: normal.normalize(),
            width,
            height,
            density,
        }
    }

    /// 生成表面采样点
    pub fn sample_points(&self) -> Points<f64> {
        match self {
            Sharps3::CylinderSurface {
                center,
                radius,
                height,
                density,
            } => simple_cylinder_surface(*center, *radius, *height, *density),
            Sharps3::SphereSurface {
                center,
                radius,
                density,
            } => sample_sphere_surface(*center, *radius, *density),
            Sharps3::Plane {
                point,
                normal,
                width,
                height,
                density,
            } => sample_plane_surface(*point, *normal, *width, *height, *density),
        }
    }
}

/// 默认圆柱表面采样（保留原实现，稍作优化）
pub fn simple_cylinder_surface(
    center: Point3<f64>,
    radius: f64,
    height: f64,
    density: f64,
) -> Points<f64> {
    let mut points = Vec::new();
    let side_area = 2.0 * PI * radius * height;
    let cap_area = PI * radius * radius;
    let side_samples = (side_area * density).max(1.0) as usize;
    let cap_samples = (cap_area * density).max(1.0) as usize;

    // 侧面
    for i in 0..side_samples {
        let frac = i as f64 / side_samples as f64;
        let theta = frac * 2.0 * PI;
        let z = frac * height;
        let x = center.x + radius * theta.cos();
        let y = center.y + radius * theta.sin();
        let z_pos = center.z + z;
        points.push(Point3::new(x, y, z_pos));
    }

    // 底面（黄金角方法）
    let golden_angle = 2.399963;
    for i in 0..cap_samples {
        let r = radius * (i as f64 / cap_samples as f64).sqrt();
        let theta = i as f64 * golden_angle;
        let x = center.x + r * theta.cos();
        let y = center.y + r * theta.sin();
        points.push(Point3::new(x, y, center.z));
    }

    // 顶面
    for i in 0..cap_samples {
        let r = radius * (i as f64 / cap_samples as f64).sqrt();
        let theta = i as f64 * golden_angle;
        let x = center.x + r * theta.cos();
        let y = center.y + r * theta.sin();
        points.push(Point3::new(x, y, center.z + height));
    }

    points
}

/// 球体表面斐波那契采样
fn sample_sphere_surface(center: Point3<f64>, radius: f64, density: f64) -> Points<f64> {
    let area = 4.0 * PI * radius * radius;
    let n = (area * density).max(1.0) as usize;
    let mut points = Vec::with_capacity(n);
    let golden_ratio = (1.0 + 5.0_f64.sqrt()) / 2.0;

    for i in 0..n {
        let theta = 2.0 * PI * (i as f64) / golden_ratio;
        let phi = (1.0 - 2.0 * (i as f64 + 0.5) / n as f64).acos();
        let x = center.x + radius * phi.sin() * theta.cos();
        let y = center.y + radius * phi.sin() * theta.sin();
        let z = center.z + radius * phi.cos();
        points.push(Point3::new(x, y, z));
    }
    points
}

/// 平面规则网格采样
fn sample_plane_surface(
    point: Point3<f64>,
    normal: Vector3<f64>,
    width: f64,
    height: f64,
    density: f64,
) -> Points<f64> {
    let area = width * height;
    let num_samples = (area * density).max(1.0) as usize;
    let mut points = Vec::with_capacity(num_samples);

    let up = Vector3::new(0.0, 0.0, 1.0);
    let u = if normal.dot(&up).abs() > 0.99 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        normal.cross(&up).normalize()
    };
    let v = normal.cross(&u).normalize();

    let cols = (num_samples as f64).sqrt().ceil() as usize;
    let rows = (num_samples as f64 / cols as f64).ceil() as usize;

    for i in 0..cols {
        for j in 0..rows {
            let u_frac = if cols > 1 { i as f64 / (cols - 1) as f64 } else { 0.5 };
            let v_frac = if rows > 1 { j as f64 / (rows - 1) as f64 } else { 0.5 };
            let u_offset = (u_frac - 0.5) * width;
            let v_offset = (v_frac - 0.5) * height;
            let pos = point + u * u_offset + v * v_offset;
            points.push(pos);
        }
    }
    points.truncate(num_samples);
    points
}

// ---------------------------------------------------------------------------
// 坐标系统
// ---------------------------------------------------------------------------

/// 管理一组粒子的时空容器。
///
/// 提供粒子增删、时间推进以及批量更新功能。
/// 内部使用 `Vec<Particle3>` 存储，索引即 ID。
#[derive(Debug, Clone)]
pub struct CoordinateSystem3 {
    points: Particle3s,
    time: f64,
}

impl CoordinateSystem3 {
    /// 创建空坐标系统，时间从 0 开始。
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            time: 0.0,
        }
    }

    /// 设置初始时间（Builder 模式）
    pub fn with_time(mut self, t: f64) -> Self {
        self.time = t;
        self
    }

    /// 添加粒子，返回其索引
    pub fn add_particle(&mut self, particle: Particle3) -> usize {
        let idx = self.points.len();
        self.points.push(particle);
        idx
    }

    /// 批量添加粒子，返回索引列表
    pub fn add_particles(&mut self, particles: Vec<Particle3>) -> Vec<usize> {
        let start = self.points.len();
        self.points.extend(particles);
        (start..self.points.len()).collect()
    }

    /// 根据索引移除粒子（交换删除，不保持顺序）
    pub fn remove_particle(&mut self, index: usize) -> Option<Particle3> {
        if index < self.points.len() {
            Some(self.points.swap_remove(index))
        } else {
            None
        }
    }

    /// 清空所有粒子
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// 获取粒子数量
    pub fn particle_count(&self) -> usize {
        self.points.len()
    }

    /// 获取当前模拟时间
    pub fn time(&self) -> f64 {
        self.time
    }

    /// 设置模拟时间
    pub fn set_time(&mut self, t: f64) {
        self.time = t;
    }

    /// 标准运动学更新：推进所有粒子的位置和速度，增加时间
    pub fn update(&mut self, dt: f64) {
        for p in &mut self.points {
            p.update(dt);
        }
        self.time += dt;
    }

    /// 支持自定义更新逻辑（Rust 闭包，不适合 Python 回调）
    #[allow(dead_code)]
    pub(crate) fn update_with<F>(&mut self, dt: f64, f: F)
    where
        F: Fn(&mut Self, f64),
    {
        f(self, dt);
        self.time += dt;
    }

    /// 获取所有粒子的不可变引用
    pub fn particles(&self) -> &[Particle3] {
        &self.points
    }

    /// 获取所有粒子的可变引用
    pub fn particles_mut(&mut self) -> &mut [Particle3] {
        &mut self.points
    }

    /// 获取单个粒子的不可变引用
    pub fn get_particle(&self, index: usize) -> Option<&Particle3> {
        self.points.get(index)
    }

    /// 获取单个粒子的可变引用
    pub fn get_particle_mut(&mut self, index: usize) -> Option<&mut Particle3> {
        self.points.get_mut(index)
    }

    /// 按索引更新单个粒子（运动学推进）
    /// 如果索引有效则更新并返回 Some(())，否则 None
    pub fn update_particle(&mut self, index: usize, dt: f64) -> Option<()> {
        self.points.get_mut(index).map(|p| p.update(dt))
    }
}

impl Default for CoordinateSystem3 {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 几何工具函数
// ---------------------------------------------------------------------------

/// 从点 p1 指向 p2 的方向向量，缩放为指定长度
pub fn vector_from_points(p1: Point3<f64>, p2: Point3<f64>, val: f64) -> Vector3<f64> {
    let v = p2 - p1;
    if let Some(n) = v.try_normalize(f64::EPSILON) {
        n * val
    } else {
        Vector3::zeros()
    }
}

/// 判断点 p3 到线段 p1-p2 的最短距离是否 ≤ d
pub fn is_within_distance(
    p1: &Point3<f64>,
    p2: &Point3<f64>,
    p3: &Point3<f64>,
    d: f64,
) -> bool {
    let v2 = p2 - p1;
    let line_len_sq = v2.norm_squared();

    if line_len_sq < f64::EPSILON {
        return (p3 - p1).norm_squared() <= d * d;
    }

    let v1 = p3 - p1;
    let t = v1.dot(&v2) / line_len_sq;

    let closest_dist_sq = if t <= 0.0 {
        v1.norm_squared()
    } else if t >= 1.0 {
        (p3 - p2).norm_squared()
    } else {
        v1.cross(&v2).norm_squared() / line_len_sq
    };
    closest_dist_sq <= d * d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_to_segment() {
        let p1 = Point3::new(0.0, 0.0, 0.0);
        let p2 = Point3::new(10.0, 0.0, 0.0);
        let p3 = Point3::new(5.0, 1.0, 0.0);
        assert!(is_within_distance(&p1, &p2, &p3, 1.0));
        assert!(!is_within_distance(&p1, &p2, &p3, 0.5));
    }

    #[test]
    fn test_cylinder_sampling() {
        let pts = simple_cylinder_surface(
            Point3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            10.0,
        );
        assert!(!pts.is_empty());
        // 所有点应在圆柱表面附近
        for p in pts {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!((r - 1.0).abs() < 1e-6 || (p.z - 0.0).abs() < 1e-6 || (p.z - 2.0).abs() < 1e-6);
        }
    }
}