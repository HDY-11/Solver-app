use nalgebra::{Point3, Vector3};
use std::f64::consts::PI;
use std::collections::HashSet;

type Particle3s = Vec<Particle3>;
/// 点集类型
pub type Points<T> = Vec<Point3<T>>;


/// 单位m/s^2
pub static G: f64 = 9.8;
#[derive(Debug, Clone)]
pub struct Particle3{
    pub id : usize,
    /// 单位m
    pub position : Point3<f64>,
    /// 单位kg
    pub m : f64,
    /// 单位m/s
    pub v : Vector3<f64>,
    /// 单位m/s^2
    pub a : Vector3<f64>,
    /// 最晚更新时刻
    pub t : f64,
}

impl Particle3{
    pub fn new(p : Point3<f64>, id: usize)->Self{
        Self{
            id,
            position: p,
            m: 0.0,
            v: Vector3::new(0.0, 0.0, 0.0),
            a: Vector3::new(0.0, 0.0, 0.0),
            t: 0.0,
        }
    }
    pub fn with_velocity(mut self, v: Vector3<f64>)->Self{
        self.v = v;
        self
    }
    pub fn with_acceleration(mut self, a: Vector3<f64>)->Self{
        self.a = a;
        self
    }
    pub fn with_mass(mut self, m: f64)->Self{
        assert!(m >= 0.0, "质量必须为非负数");
        self.m = m;
        self
    }
    pub fn with_lastest_time(mut self, t:f64)->Self{
        self.t = t;
        self
    }
    pub fn update(&mut self, dt : f64){
        self.update_velocity(dt);
        self.update_position(dt);
        self.t += dt;
    }
    pub fn update_position(&mut self, dt : f64){
        self.position += self.v * dt + 0.5 * self.a * dt * dt;
    }
    pub fn update_velocity(&mut self, dt : f64){
        self.v += self.a * dt;
    }
    pub fn update_position_with(&mut self, a : Vector3<f64>, dt : f64){
        self.position += self.v * dt + 0.5 * a * dt * dt;
    }
    pub fn update_velocity_with(&mut self, a : Vector3<f64>, dt : f64){
        self.v += a * dt;
    }
    pub fn update_with_force(&mut self, force : Vector3<f64>, dt : f64){
        assert!(self.m > 0.0, "质量必须为正数");
        let a = (force / self.m) + self.a;
        self.update_velocity_with(a, dt);
        self.update_position_with(a, dt);
        self.t += dt;
    }
    pub fn update_with_acceleration(&mut self, a : Vector3<f64>, dt : f64){
        let a = a + self.a;
        self.update_velocity_with(a, dt);
        self.update_position_with(a, dt);
        self.t += dt;
    }
    pub fn apply_force(&mut self, force : Vector3<f64>){
        let a = force / self.m;
        self.a += a;
    }
}

/// 形状枚举
#[derive(Debug, Clone)]
pub enum Sharps3{
    /// **圆柱表面**
    /// 
    /// 参数：
    /// 
    /// o: 下底面中心点
    /// r: 半径(m)
    /// h: 高度(m)
    /// density: 每平方米的采样点数
    /// 
    /// 返回值：圆柱表面离散采样的点集
    CylinderSurface {
        center: Point3<f64>,
        radius: f64,
        height: f64,
        density: f64,
    },
    /// **球体表面**
    /// 
    /// 参数：
    /// 
    /// o: 球心点
    /// r: 半径(m)
    /// density: 每平方米的采样点数
    /// 
    /// 返回值：球体表面离散采样的点集
    SphereSurface {
        center: Point3<f64>,
        radius: f64,
        density: f64,
    },
    /// **平面**
    /// 
    /// 参数：
    /// 
    /// o: 平面上一点
    /// n: 平面法向量
    /// w: 平面宽度(m)
    /// h: 平面高度(m)
    /// density: 每平方米的采样点数
    /// 
    /// 返回值：平面离散采样的点集
    Plane {
        point: Point3<f64>,
        normal: Vector3<f64>,
        width: f64,
        height: f64,
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
    pub fn plane(point: Point3<f64>, normal: Vector3<f64>, width: f64, height: f64, density: f64) -> Self {
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
            Sharps3::CylinderSurface { center, radius, height, density} => {
                simple_cylinder_surface(*center, *radius, *height, *density)
            }
            Sharps3::SphereSurface { center, radius, density } => {
                sample_sphere_surface(*center, *radius, *density)
            }
            Sharps3::Plane { point, normal, width, height, density } => {
                sample_plane_surface(*point, *normal, *width, *height, *density)
            }
        }
    }
}

/// 默认圆柱表面采样函数
pub fn simple_cylinder_surface(center: Point3<f64>, radius: f64, height: f64, density: f64) -> Points<f64> {
    let mut points = Vec::new();
    
    // 计算采样点数
    let side_area = 2.0 * PI * radius * height;
    let cap_area = PI * radius * radius;
    let side_samples = (side_area * density) as usize;
    let cap_samples = (cap_area * density) as usize;
    
    // 采样侧面
    for i in 0..side_samples {
        let theta = (i as f64 / side_samples as f64) * 2.0 * PI;
        let z = (i as f64 / side_samples as f64) * height;
        
        let x = center.x + radius * theta.cos();
        let y = center.y + radius * theta.sin();
        let z = center.z + z;
        
        points.push(Point3::new(x, y, z));
    }
    
    // 采样下底面
    for i in 0..cap_samples {
        let r = radius * ((i as f64) / (cap_samples as f64)).sqrt();
        let theta = (i as f64) * 2.399963; // 黄金角
        
        let x = center.x + r * theta.cos();
        let y = center.y + r * theta.sin();
        let z = center.z;
        
        points.push(Point3::new(x, y, z));
    }
    
    // 采样上底面
    for i in 0..cap_samples {
        let r = radius * ((i as f64) / (cap_samples as f64)).sqrt();
        let theta = (i as f64) * 2.399963;
        
        let x = center.x + r * theta.cos();
        let y = center.y + r * theta.sin();
        let z = center.z + height;
        
        points.push(Point3::new(x, y, z));
    }
    
    points
}

/// 球体表面采样
fn sample_sphere_surface(center: Point3<f64>, radius: f64, density: f64) -> Points<f64> {
    let mut points = Vec::new();
    let area = 4.0 * PI * radius * radius;
    let num_samples = (area * density) as usize;
    
    for i in 0..num_samples {
        // 使用斐波那契球体采样
        let golden_ratio = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let theta = 2.0 * PI * (i as f64) / golden_ratio;
        let phi = (1.0 - 2.0 * (i as f64 + 0.5) / num_samples as f64).acos();
        
        let x = center.x + radius * phi.sin() * theta.cos();
        let y = center.y + radius * phi.sin() * theta.sin();
        let z = center.z + radius * phi.cos();
        
        points.push(Point3::new(x, y, z));
    }
    
    points
}

/// 平面表面采样
fn sample_plane_surface(point: Point3<f64>, normal: Vector3<f64>, width: f64, height: f64, density: f64) -> Points<f64> {
    let mut points = Vec::new();
    let area = width * height;
    let num_samples = (area * density) as usize;
    
    // 构建局部坐标系
    let up = Vector3::new(0.0, 0.0, 1.0);
    let u = if normal.dot(&up).abs() > 0.99 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        normal.cross(&up).normalize()
    };
    let v = normal.cross(&u).normalize();
    
    let samples_per_row = (num_samples as f64).sqrt() as usize;
    let samples_per_col = num_samples / samples_per_row;
    
    for i in 0..samples_per_row {
        for j in 0..samples_per_col {
            let u_offset = (i as f64 / samples_per_row as f64 - 0.5) * width;
            let v_offset = (j as f64 / samples_per_col as f64 - 0.5) * height;
            
            let pos = point + u * u_offset + v * v_offset;
            points.push(pos);
        }
    }
    
    points
}

#[derive(Debug, Clone)]
pub struct ParticleSystem3{
    pub points: Particle3s,
    pub core: Particle3,
    pub shape: Sharps3
}

#[derive(Debug, Clone)]
pub struct CoordinateSystem3 {
    points: Particle3s,
    plist: HashSet<usize>,
    time: f64,
}

impl CoordinateSystem3 {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            plist: HashSet::new(),
            time: 0.0,
        }
    }
    
    pub fn with_time(mut self, t: f64)->Self{
        self.time = t;
        self
    }

    /// 添加粒子
    pub fn add_particle(&mut self, particle: Particle3) -> usize {
        let id = self.points.len();
        self.points.push(particle);
        id
    }
    
    /// 批量添加粒子
    pub fn add_particles(&mut self, particles: Vec<Particle3>) -> Vec<usize> {
        let start_id = self.points.len();
        self.points.extend(particles);
        (start_id..self.points.len()).collect()
    }
    
    /// 获取当前时间
    pub fn time(&self) -> f64 {
        self.time
    }
    
    /// 更新系统
    pub fn update(&mut self, dt: f64) {
        for particle in &mut self.points {
            particle.update(dt);
        }
        self.time += dt;
    }
    pub fn update_with<F>(&mut self, dt: f64, f : F)
    where F : Fn(&mut Self, f64){
        f(self, dt);
        self.time += dt;
    }
    
    pub fn particles(&self) -> &[Particle3] {
        &self.points
    }
    
    pub fn particles_mut(&mut self) -> &mut [Particle3] {
        &mut self.points
    }
}

impl Default for CoordinateSystem3 {
    fn default() -> Self {
        Self::new()
    }
}

pub fn vector_from_points(p1: Point3<f64>, p2: Point3<f64>, val: f64) -> Vector3<f64> {
    let v = Vector3::new(p2.x - p1.x, p2.y - p1.y, p2.z - p1.z);
    v.normalize() * val
}

// 判断，点p1,p2形成的线段与p3的距离是否小于等于d
pub fn is_within_distance(
    p1: &Point3<f64>,
    p2: &Point3<f64>,
    p3: &Point3<f64>,
    d: f64,
) -> bool {
    let v2 = p2 - p1;                // 线段方向向量
    let line_len_sq = v2.norm_squared();

    // 退化情况：线段端点重合，退化为点
    if line_len_sq < f64::EPSILON {
        return (p3 - p1).norm_squared() <= d * d;
    }

    let v1 = p3 - p1;

    // 计算投影参数 t = (v1 · v2) / (v2 · v2)
    let t = v1.dot(&v2) / line_len_sq;
    // 根据 t 的范围确定最近点
    let closest_dist_sq = if t <= 0.0 {
        // 最近点为 p1
        v1.norm_squared()
    } else if t >= 1.0 {
        // 最近点为 p2
        (p3 - p2).norm_squared()
    } else {
        // 最近点在线段内部，使用叉积计算垂线距离平方
        // 距离^2 = |v1 × v2|^2 / |v2|^2
        v1.cross(&v2).norm_squared() / line_len_sq
    };
    closest_dist_sq <= d * d
}
#[test]
fn main(){
    let mut system = CoordinateSystem3::new().with_time(5.1);
    // 安排质点
    let mut m1 = Particle3::new(Point3::new(20000.0, 0.0, 2000.0), 0)
        .with_velocity(-vector_from_points(Point3::new(0.0, 0.0, 0.0), Point3::new(20000.0, 0.0, 2000.0), 300.0));
    let mut ymd: Particle3 = Particle3::new(Point3::new(17188.0, 0.0, 1736.496), 0)
        .with_velocity(Vector3::new(0.0, 0.0, -3.0))
        .with_lastest_time(5.1);

    // 模拟计算阶段1：M1的在5.1s后的位置
    m1.update(5.1);

    system.add_particle(m1);
    system.add_particle(ymd);

    // 模拟
    // 阶段2：5.1s起爆之后,到20s后导弹失效
    const DT: f64 = 0.0001;
    let sharp1 = Sharps3::cylinder_surface(
        Point3::new(0.0, 200.0, 0.0), 7.0, 10.0, 0.1
    );
    let points1 = sharp1.sample_points();


    let mut result = 0.0;
    for _ in 0..200000 {
        system.update(DT);
        let m1 = &system.particles()[0];
        let ymd = &system.particles()[1];
        let mut count = 0.0;
        for s in points1.iter() {
            if is_within_distance(&m1.position, s, &ymd.position, 10.0) {
                count += 1.0;
            }
        }
        if count/points1.len() as f64 > 0.95 {
            result += DT;
        }
        println!("[{}s 时刻] M1位置: {:?}, 烟幕圆心位置: {:?}, 遮蔽时长: {}, 遮蔽率: {:.2}%", 
            system.time(),
            m1.position, 
            ymd.position, 
            result,
            count/points1.len() as f64 * 100.0
        );
    }
}