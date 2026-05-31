//! 预定义力模型，用于在 Rust 侧高效更新粒子。
//!
//! 这些模型覆盖常见的物理场景，避免在热循环中跨越 FFI 调用 Python 函数。
//! 用户可以在 Python 侧通过枚举变体选择模型，然后将结构传给 `Motion`。

use nalgebra::{Point3, Vector3};
use crate::coordinate_system::{CoordinateSystem3, G, Particle3};

/// 可在整个系统上施加的力模型。
///
/// # 示例（Rust）
/// ```
/// use simple_motion::force_models::ForceModel;
/// let gravity = ForceModel::uniform_gravity(9.8, [0.0, 0.0, -1.0]);
/// ```
#[derive(Debug, Clone)]
pub enum ForceModel {
    /// 均匀重力场：所有粒子受到恒定加速度。
    UniformGravity {
        /// 加速度大小 (m/s²)
        g: f64,
        /// 方向单位向量（自动归一化）
        direction: Vector3<f64>,
    },
    /// 空气阻力：F = -0.5 * rho * Cd * A * |v| * v。
    /// 这里简化为 F = -k * |v| * v（k 为阻尼系数）。
    Drag {
        /// 阻尼系数 (kg/m)
        coefficient: f64,
    },
    /// 弹簧力：F = -k * (x - x0) 作用在指定粒子上。
    /// 锚点可以是静态点或另一个粒子的索引。
    Spring {
        /// 弹簧刚度 (N/m)
        stiffness: f64,
        /// 原长 (m)
        rest_length: f64,
        /// 锚点（可以是固定点或粒子索引）
        anchor: Anchor,
    },
    /// 库仑力：F = k * q1 * q2 / r² * r_hat（作用于两个粒子之间）
    Coulomb {
        /// 库仑常数 (N·m²/C²)
        k: f64,
        /// 粒子1 索引
        particle1: usize,
        /// 粒子2 索引
        particle2: usize,
    },
}

/// 弹簧锚点位置
#[derive(Debug, Clone)]
pub enum Anchor {
    /// 固定空间点
    Fixed(Point3<f64>),
    /// 另一个粒子的索引
    Particle(usize),
}

impl ForceModel {
    /// 创建均匀重力场
    pub fn uniform_gravity(g: f64, direction: [f64; 3]) -> Self {
        ForceModel::UniformGravity {
            g,
            direction: Vector3::new(direction[0], direction[1], direction[2]).normalize(),
        }
    }

    /// 创建线性空气阻力
    pub fn drag(coefficient: f64) -> Self {
        ForceModel::Drag { coefficient }
    }

    /// 创建弹簧力，锚点为固定点
    pub fn spring_fixed(stiffness: f64, rest_length: f64, anchor: Point3<f64>) -> Self {
        ForceModel::Spring {
            stiffness,
            rest_length,
            anchor: Anchor::Fixed(anchor),
        }
    }

    /// 创建弹簧力，锚点为另一粒子
    pub fn spring_particle(stiffness: f64, rest_length: f64, anchor_index: usize) -> Self {
        ForceModel::Spring {
            stiffness,
            rest_length,
            anchor: Anchor::Particle(anchor_index),
        }
    }

    /// 创建库仑力
    pub fn coulomb(k: f64, p1: usize, p2: usize) -> Self {
        ForceModel::Coulomb { k, particle1: p1, particle2: p2 }
    }

    /// 将该力模型应用到整个系统，更新所有相关粒子的加速度。
    ///
    /// 注意：不会清除原有加速度，而是累积。
    pub fn apply(&self, system: &mut CoordinateSystem3) {
        match self {
            ForceModel::UniformGravity { g, direction } => {
                let acc = direction * (*g);
                for p in system.particles_mut() {
                    p.a += acc;
                }
            }
            ForceModel::Drag { coefficient } => {
                for p in system.particles_mut() {
                    if p.m <= 0.0 { continue; }
                    let speed = p.v.norm();
                    if speed > 0.0 {
                        let force = -p.v.normalize() * *coefficient * speed * speed;
                        p.apply_force(force);
                    }
                }
            }
            ForceModel::Spring { stiffness, rest_length, anchor } => {
                let anchor_pos = match anchor {
                    Anchor::Fixed(pos) => *pos,
                    Anchor::Particle(idx) => {
                        if let Some(anchor_p) = system.get_particle(*idx) {
                            anchor_p.position
                        } else {
                            return;
                        }
                    }
                };

                for p in system.particles_mut() {
                    let dir = p.position - anchor_pos;
                    let dist = dir.norm();
                    if dist < f64::EPSILON { continue; }
                    let force = -dir.normalize() * *stiffness * (dist - *rest_length);
                    p.apply_force(force);
                }
            }
            ForceModel::Coulomb { k, particle1, particle2 } => {
                if *particle1 == *particle2 { return; }
                let idx1 = *particle1;
                let idx2 = *particle2;
                let particles = system.particles_mut();
                if idx1 >= particles.len() || idx2 >= particles.len() { return; }

                // 使用 split_at_mut 安全获取两个可变引用
                let (first, second) = if idx1 < idx2 {
                    let (left, right) = particles.split_at_mut(idx2);
                    (&mut left[idx1], &mut right[0])
                } else {
                    let (left, right) = particles.split_at_mut(idx1);
                    (&mut right[0], &mut left[idx2])
                };

                let r_vec = second.position - first.position;
                let r = r_vec.norm();
                if r < f64::EPSILON { return; }
                let force = r_vec.normalize() * (k / (r * r));
                first.apply_force(-force);
                second.apply_force(force);
            }
        }
    }
}