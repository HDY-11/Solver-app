//! `simple_motion` Python 模块。
//!
//! 提供高性能的物理模拟核心，包括：
//! - [`Motion`] 遥控器类：管理粒子系统和预定义力模型。
//! - [`Particle3`] 粒子构造器。
//! - 几何采样函数，直接返回 NumPy 数组。
//! - 遮蔽率计算函数（Rust 侧高性能）。

pub mod coordinate_system;
pub mod force_models;
pub mod batch;

use nalgebra::{ Vector3 };
use coordinate_system::*;
use force_models::ForceModel;
use batch as batch_ops;

use nalgebra::Point3;
use numpy::{PyArray1, PyArray2, PyArrayMethods};
use pyo3::prelude::*;
use std::sync::{Arc, Mutex};

/// 线程安全的共享类型
pub type Shared<T> = Arc<Mutex<T>>;

// =========================================================================
// Python 可见的粒子构造器
// =========================================================================

/// Python 可见的粒子构造器（不可变风格）。
///
/// 在 Python 中可通过链式调用设置属性，最后添加到 [`Motion`] 中。
///
/// # 示例
/// ```python
/// p = Particle3(1.0, 2.0, 3.0, id=1).set_velocity(0.0, 0.0, -10.0).set_mass(5.0)
/// ```
#[pyclass(name = "Particle3")]
#[derive(Clone)]
pub struct PyParticle3 {
    inner: Particle3,
}

#[pymethods]
impl PyParticle3 {
    /// 创建新粒子。
    ///
    /// Args:
    ///     x, y, z: 初始位置 (m)
    ///     id: 用户标识符
    #[new]
    #[pyo3(signature = (x, y, z, id=0))]
    pub fn new(x: f64, y: f64, z: f64, id: usize) -> Self {
        Self {
            inner: Particle3::new(Point3::new(x, y, z), id),
        }
    }

    /// 设置速度（返回新对象，不修改原对象）
    #[pyo3(name = "set_velocity")]
    pub fn set_velocity(&self, vx: f64, vy: f64, vz: f64) -> Self {
        let mut cloned = self.inner.clone();
        cloned.v = Vector3::new(vx, vy, vz);
        Self { inner: cloned }
    }

    /// 设置加速度（固有）
    #[pyo3(name = "set_acceleration")]
    pub fn set_acceleration(&self, ax: f64, ay: f64, az: f64) -> Self {
        let mut cloned = self.inner.clone();
        cloned.a = Vector3::new(ax, ay, az);
        Self { inner: cloned }
    }

    /// 设置质量
    #[pyo3(name = "set_mass")]
    pub fn set_mass(&self, m: f64) -> PyResult<Self> {
        if m < 0.0 {
            return Err(pyo3::exceptions::PyValueError::new_err("质量必须为非负数"));
        }
        let mut cloned = self.inner.clone();
        cloned.m = m;
        Ok(Self { inner: cloned })
    }

    /// 设置最后更新时间
    #[pyo3(name = "set_lastest_time")]
    pub fn set_lastest_time(&self, t: f64) -> Self {
        let mut cloned = self.inner.clone();
        cloned.t = t;
        Self { inner: cloned }
    }

    // 属性访问
    #[getter]
    pub fn id(&self) -> usize {
        self.inner.id
    }

    #[getter]
    pub fn position(&self) -> (f64, f64, f64) {
        (self.inner.position.x, self.inner.position.y, self.inner.position.z)
    }

    #[getter]
    pub fn velocity(&self) -> (f64, f64, f64) {
        (self.inner.v.x, self.inner.v.y, self.inner.v.z)
    }

    #[getter]
    pub fn acceleration(&self) -> (f64, f64, f64) {
        (self.inner.a.x, self.inner.a.y, self.inner.a.z)
    }

    #[getter]
    pub fn mass(&self) -> f64 {
        self.inner.m
    }

    #[getter]
    pub fn time(&self) -> f64 {
        self.inner.t
    }

    fn __repr__(&self) -> String {
        format!(
            "Particle3(id={}, pos=({:.3}, {:.3}, {:.3}), v=({:.3}, {:.3}, {:.3}))",
            self.inner.id,
            self.inner.position.x, self.inner.position.y, self.inner.position.z,
            self.inner.v.x, self.inner.v.y, self.inner.v.z
        )
    }
}

// =========================================================================
// Python 可见的力模型
// =========================================================================

/// Python 可见的力模型构造器（不可变，纯数据）
#[pyclass(name = "ForceModel")]
#[derive(Clone)]
pub struct PyForceModel {
    inner: ForceModel,
}

#[pymethods]
impl PyForceModel {
    /// 均匀重力场
    #[staticmethod]
    #[pyo3(signature = (g, direction))]
    pub fn uniform_gravity(g: f64, direction: [f64; 3]) -> Self {
        Self {
            inner: ForceModel::uniform_gravity(g, direction),
        }
    }

    /// 空气阻力
    #[staticmethod]
    pub fn drag(coefficient: f64) -> Self {
        Self {
            inner: ForceModel::drag(coefficient),
        }
    }

    /// 弹簧力（固定锚点）
    #[staticmethod]
    #[pyo3(signature = (stiffness, rest_length, anchor_x, anchor_y, anchor_z))]
    pub fn spring_fixed(
        stiffness: f64,
        rest_length: f64,
        anchor_x: f64,
        anchor_y: f64,
        anchor_z: f64,
    ) -> Self {
        Self {
            inner: ForceModel::spring_fixed(
                stiffness,
                rest_length,
                Point3::new(anchor_x, anchor_y, anchor_z),
            ),
        }
    }

    /// 弹簧力（粒子锚点）
    #[staticmethod]
    pub fn spring_particle(stiffness: f64, rest_length: f64, anchor_index: usize) -> Self {
        Self {
            inner: ForceModel::spring_particle(stiffness, rest_length, anchor_index),
        }
    }

    /// 库仑力
    #[staticmethod]
    pub fn coulomb(k: f64, p1: usize, p2: usize) -> Self {
        Self {
            inner: ForceModel::coulomb(k, p1, p2),
        }
    }

    fn __repr__(&self) -> String {
        format!("ForceModel({:?})", self.inner)
    }
}

// =========================================================================
// 遥控器 Motion
// =========================================================================

/// Python 前端的核心遥控器。
///
/// 内部使用 `Arc<Mutex<CoordinateSystem3>>` 以保证未来多线程安全性。
///
/// # 示例
/// ```python
/// motion = Motion(time=5.1)
/// p1 = Particle3(0,0,0, id=0).set_velocity(1,0,0)
/// motion.add_particle(p1)
/// motion.update(0.01)
/// positions = motion.get_positions()  # numpy array
/// ```
#[pyclass(name = "Motion")]
pub struct Motion {
    inner: Shared<CoordinateSystem3>,
}

#[pymethods]
impl Motion {
    /// 创建新系统
    #[new]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CoordinateSystem3::new())),
        }
    }

    // ----- 粒子管理 -----
    /// 添加粒子，返回索引
    pub fn add_particle(&self, particle: &PyParticle3) -> PyResult<usize> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(sys.add_particle(particle.inner.clone()))
    }

    /// 移除粒子
    pub fn remove_particle(&self, index: usize) -> PyResult<()> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        sys.remove_particle(index);
        Ok(())
    }

    /// 清空所有粒子
    pub fn clear(&self) -> PyResult<()> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        sys.clear();
        Ok(())
    }

    /// 粒子数量
    #[getter]
    pub fn particle_count(&self) -> PyResult<usize> {
        let sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(sys.particle_count())
    }

    // ----- 时间 -----
    /// 获取当前时间
    #[getter]
    pub fn time(&self) -> PyResult<f64> {
        let sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(sys.time())
    }

    /// 设置当前时间
    pub fn set_time(&self, t: f64) -> PyResult<()> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        sys.set_time(t);
        Ok(())
    }

    // ----- 基本更新 -----
    /// 标准运动学更新（不考虑外力）
    pub fn update(&self, dt: f64) -> PyResult<()> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        sys.update(dt);
        Ok(())
    }

    /// 应用力模型到系统（累积加速度），通常随后调用 `update`。
    pub fn apply_forces(&self, forces: Vec<PyForceModel>) -> PyResult<()> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        for f in forces {
            f.inner.apply(&mut sys);
        }
        Ok(())
    }

    /// 重置所有粒子的加速度为 0（通常在每帧开始时调用）
    pub fn reset_accelerations(&self) -> PyResult<()> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        for p in sys.particles_mut() {
            p.a = Vector3::zeros();
        }
        Ok(())
    }

    // ----- 批量数据接口 (NumPy) -----
    /// 获取所有粒子的位置，形状 (N,3) 的 float64 数组。
    pub fn get_positions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let data = batch_ops::positions_matrix(&sys);
        let n = sys.particle_count();
        Ok(PyArray2::from_vec2(py, &vec![data])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            .reshape([n, 3])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?)
    }

    /// 获取所有粒子的速度，形状 (N,3) 的 float64 数组。
    pub fn get_velocities<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let data = batch_ops::velocities_matrix(&sys);
        let n = sys.particle_count();
        Ok(PyArray2::from_vec2(py, &vec![data])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            .reshape([n, 3])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?)
    }

    /// 获取所有粒子的质量，形状 (N,) 的 float64 数组。
    pub fn get_masses<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let data = batch_ops::masses_vector(&sys);
        Ok(PyArray1::from_vec(py, data))
    }

    /// 设置所有粒子的加速度，形状 (N,3) 的 float64 数组。
    /// （通常与 Python 自定义力计算配合使用）
    pub fn set_accelerations<'py>(
        &self,
        py: Python<'py>,
        acc: Bound<'py, PyArray2<f64>>,
    ) -> PyResult<()> {
        let acc = acc.readonly();
        let acc_slice = acc.as_slice()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let n = sys.particle_count();
        if acc_slice.len() != n * 3 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                format!("expected {} elements, got {}", n * 3, acc_slice.len())
            ));
        }
        for (i, p) in sys.particles_mut().iter_mut().enumerate() {
            let base = i * 3;
            p.a = Vector3::new(acc_slice[base], acc_slice[base + 1], acc_slice[base + 2]);
        }
        Ok(())
    }

    /// 自定义运动学更新：使用 Python 侧计算好的加速度，然后推进位置和速度。
    ///
    /// 等效于：先 `set_accelerations(acc)`，再调用 `update(dt)`。
    /// 提供此方法是为了减少 FFI 调用次数。
    pub fn update_with_accelerations<'py>(
        &self,
        py: Python<'py>,
        dt: f64,
        acc: Bound<'py, PyArray2<f64>>,
    ) -> PyResult<()> {
        self.set_accelerations(py, acc)?;
        self.update(dt)
    }

    // ----- 几何采样 (NumPy) -----
    /// 生成圆柱表面采样点，返回 (M,3) 的 NumPy 数组
    #[staticmethod]
    pub fn cylinder_surface_points<'py>(
        py: Python<'py>,
        cx: f64, cy: f64, cz: f64,
        radius: f64,
        height: f64,
        density: f64,
    ) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let shape = Sharps3::cylinder_surface(Point3::new(cx, cy, cz), radius, height, density);
        let pts = shape.sample_points();
        let data = batch_ops::points_to_matrix(&pts);
        let n = pts.len();
        Ok(PyArray2::from_vec2(py, &vec![data])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            .reshape([n, 3])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?)
    }

    /// 生成球体表面采样点
    #[staticmethod]
    pub fn sphere_surface_points<'py>(
        py: Python<'py>,
        cx: f64, cy: f64, cz: f64,
        radius: f64,
        density: f64,
    ) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let shape = Sharps3::sphere_surface(Point3::new(cx, cy, cz), radius, density);
        let pts = shape.sample_points();
        let data = batch_ops::points_to_matrix(&pts);
        let n = pts.len();
        Ok(PyArray2::from_vec2(py, &vec![data])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            .reshape([n, 3])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?)
    }

    /// 生成平面采样点
    #[staticmethod]
    pub fn plane_surface_points<'py>(
        py: Python<'py>,
        px: f64, py_: f64, pz: f64,
        nx: f64, ny: f64, nz: f64,
        width: f64,
        height: f64,
        density: f64,
    ) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let shape = Sharps3::plane(
            Point3::new(px, py_, pz),
            Vector3::new(nx, ny, nz),
            width, height, density,
        );
        let pts = shape.sample_points();
        let data = batch_ops::points_to_matrix(&pts);
        let n = pts.len();
        Ok(PyArray2::from_vec2(py, &vec![data])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            .reshape([n, 3])
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?)
    }

    // ----- 遮蔽率计算 -----
    /// 计算遮蔽率（Rust 侧循环，高效）
    ///
    /// Args:
    ///     m1_pos: (x,y,z) M1 位置
    ///     ymd_pos: (x,y,z) 烟幕粒子位置
    ///     sample_points: NumPy 数组 (M,3) 采样点
    ///     threshold: 距离阈值
    /// Returns:
    ///     float: 被遮蔽的采样点比例
    #[staticmethod]
    pub fn occlusion_ratio(
        m1_pos: (f64, f64, f64),
        ymd_pos: (f64, f64, f64),
        sample_points: Bound<PyArray2<f64>>,
        threshold: f64,
    ) -> PyResult<f64> {
        let sample_points = sample_points.readonly();
        let slice = sample_points.as_slice()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let m = slice.len() / 3;
        let mut points = Vec::with_capacity(m);
        for i in 0..m {
            let base = i * 3;
            points.push(Point3::new(slice[base], slice[base + 1], slice[base + 2]));
        }
        let m1 = Point3::new(m1_pos.0, m1_pos.1, m1_pos.2);
        let ymd = Point3::new(ymd_pos.0, ymd_pos.1, ymd_pos.2);
        Ok(batch_ops::occlusion_ratio(&m1, &ymd, &points, threshold))
    }

    /// 按索引更新单个粒子（运动学推进）
    /// 
    /// Args:
    ///     index: 粒子索引
    ///     dt: 时间步长 (s)
    /// Raises:
    ///     IndexError: 索引不存在
    ///     RuntimeError: 锁中毒
    pub fn update_particle(&self, index: usize, dt: f64) -> PyResult<()> {
        let mut sys = self.inner.lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        sys.update_particle(index, dt)
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err(format!("粒子索引 {} 不存在", index)))
    }

    fn __repr__(&self) -> String {
        if let Ok(sys) = self.inner.lock() {
            format!("Motion(time={:.3}, particles={})", sys.time(), sys.particle_count())
        } else {
            "Motion(poisoned lock)".to_string()
        }
    }
}

// =========================================================================
// 模块注册
// =========================================================================

#[pymodule]
fn simple_motion(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Motion>()?;
    m.add_class::<PyParticle3>()?;
    m.add_class::<PyForceModel>()?;
    Ok(())
}