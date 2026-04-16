pub mod coordinate_system;

use crate::coordinate_system::*;
use pyo3::{prelude::*, types::{PyFloat, PyInt}};
use nalgebra::Point3;
use std::sync::{Arc, Mutex};

pub type Shared<T> = Arc<Mutex<T>>;

#[pyclass]
struct Motion{
    pub coor_sys: Shared<CoordinateSystem3>,
}
#[pymethods]
impl Motion{
    #[new]
    pub fn new()->PyResult<Self>{
        Ok(Self{
            coor_sys : Arc::new(Mutex::new(CoordinateSystem3::new()))
        })
    }
    pub fn add_particle(&self, x: f64, y: f64, z: f64, id: Option<usize>)->PyResult<()>{
        let p= Particle3::new(Point3::new(x, y, z),id.unwrap_or(0));
        let mut g = self.coor_sys.lock().unwrap();
        g.add_particle(p);
        Ok(())
    }
    pub fn get_particle(&self, id: usize)->PyResult<Option<(f64, f64, f64)>>{
        let g = self.coor_sys.lock().unwrap();
        let p = (*g).particles().get(id);
        match p{
            Some(ps) => return Ok(Some((ps.position.x, ps.position.y, ps.position.z))),
            None => return Ok(None),
        }
    }
}

#[pymodule]
fn simple_motion(py: Python, m: &Bound<'_, PyModule>)->PyResult<()>{
    m.add_class::<Motion>()?;
    Ok(())
}