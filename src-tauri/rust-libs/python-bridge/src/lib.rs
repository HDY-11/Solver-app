use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::exceptions::{
    PyIOError, PyValueError, PyRuntimeError,
};


use std::fs;
use std::ffi::CString;
use std::path::PathBuf;
use std::env::current_exe;
use std::io::Result as IoResult;
use std::io::Error as IoError;
use std::io::ErrorKind::NotFound;
use std::collections::HashMap;


use tokio::task::spawn_blocking;


// ***********************************环境路径获取************************************
// 获取当前可执行文件所在目录，并从中构建 resources、.venv 和 scripts 的路径
// 在未找到时，panic 并输出错误信息，确保路径获取无阻塞问题，适用于 Tauri 打包后的环境



/// 获取打包后 Tauri 应用中的 resources 目录
/// 
///  所有的路径获取无阻塞问题
pub fn get_resources_path() -> PathBuf {
    let exe_dir = current_exe()
        .expect("无法获取可执行文件路径")
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    
    // 1. 首先尝试打包环境的 resources 目录
    let prod_resources = exe_dir.join("resources");
    if prod_resources.exists() {
        return prod_resources;
    }
    
    // 2. 开发环境：向上查找到项目根目录
    let mut project_root = exe_dir.clone();
    for _ in 0..3 {
        project_root = project_root
            .parent()
            .expect("无法向上查找项目根目录")
            .to_path_buf();
    }
    // 开发环境下，.venv 和 scripts 直接在项目根目录
    // 但我们仍返回项目根目录，让后续函数基于它构建路径
    project_root

}

/// 获取打包后的 .venv 路径
pub fn get_venv_path() -> PathBuf {
    let resources = get_resources_path();
    // 如果是生产环境（resources 目录存在），.venv 在 resources 下
    if resources.ends_with("resources") {
        if !resources.join(".venv").exists() {
            panic!(".venv 目录不存在: {}", resources.join(".venv").display());
        }
        return resources.join(".venv");
    }
    // 开发环境：.venv 在项目根目录
    let dev_venv = resources.join(".venv");
    if !dev_venv.exists() {
        panic!(".venv 目录不存在: {}", dev_venv.display());
    }
    dev_venv
}

/// 获取打包后 scripts 目录的路径
pub fn get_scripts_path() -> PathBuf {
    let resources = get_resources_path();
    // 如果是生产环境（resources 目录存在），.venv 在 resources 下
    if resources.ends_with("resources") {
        if !resources.join("scripts").exists() {
            panic!("Scripts 目录不存在: {}", resources.join("scripts").display());
        }
        return resources.join("scripts");
    }
    // 开发环境：scripts 在项目根目录
    let dev_scripts = resources.join("scripts");
    if !dev_scripts.exists() {
        panic!("Scripts 目录不存在: {}", dev_scripts.display());
    }
    dev_scripts
}

pub fn get_sites_packages_path() -> PathBuf {
    let venv_path = get_venv_path();
    let site_packages = venv_path.join("Lib").join("site-packages");
    if !site_packages.exists() {
        panic!("site-packages 目录不存在: {}", site_packages.display());
    }
    site_packages
}

// *******************************虚拟环境初始化******************************
/// 初始化 Python 环境，设置正确的路径
pub fn init_python_venv(py: Python) -> PyResult<()> {
    let site_packages = get_sites_packages_path();
    
    // 设置 Python 的模块搜索路径
    let sys = py.import("sys")?;
    let binding = sys.getattr("path")?;
    let sys_path: &Bound<'_, PyList> = binding.downcast()?;
    
    // 将 site-packages 插入到最前面
    sys_path.insert(0, site_packages.to_str().unwrap())?;
    
    // 将 scripts 目录也加入路径，方便导入自定义模块
    let scripts_path = get_scripts_path();
    sys_path.insert(0, scripts_path.to_str().unwrap())?;
    
    Ok(())
}
// *******************************核心功能接口******************************
/// 执行 Python 脚本
/// 
/// 使用异步+线程的方式执行，确保不会阻塞主线程
pub async fn run_script(script_path: String) -> PyResult<String> {

    let handle = spawn_blocking(move || {
        let r = Python::attach(|py| {
            init_python_venv(py)?;
        
            let script_path = PathBuf::from(script_path);
            let script_content = fs::read_to_string(&script_path)
                .map_err(|e| PyErr::new::<PyIOError, _>(
                    format!("无法读取脚本 {}: {}", script_path.display(), e)
                ))?;
        
            let script_content = CString::new(script_content)
                .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;
        
            // 捕获 stdout/stderr
            let sys = py.import("sys")?;
            let io = py.import("io")?;
            let stdout = io.call_method0("StringIO")?;
            let stderr = io.call_method0("StringIO")?;
            sys.setattr("stdout", stdout.clone())?;
            sys.setattr("stderr", stderr.clone())?;
        
            let result = py.run(&script_content, None, None);
        
            let stdout_str: String = stdout.call_method0("getvalue")?.extract()?;
            let stderr_str: String = stderr.call_method0("getvalue")?.extract()?;
        
            // 恢复原始输出流
            sys.setattr("stdout", sys.getattr("__stdout__")?)?;
            sys.setattr("stderr", sys.getattr("__stderr__")?)?;
        
            match result {
                Ok(_) => Ok(stdout_str),
                Err(e) => {
                    let error_msg = format!("{}\n{}", stderr_str, e);
                    Err(PyErr::new::<PyRuntimeError, _>(error_msg))
                }
            }
        });
        r
    });
    handle.await.map_err(|e| {
        PyErr::new::<PyRuntimeError, _>(format!("阻塞任务执行失败: {}", e))
    })?
}

pub fn save_script(code: String, file_path: String) -> IoResult<()> {
    let path = PathBuf::from(file_path);
    if !path.parent().unwrap().exists() {
        return Err(IoError::new(
            NotFound,
            format!("目录不存在: {}", path.parent().unwrap().display()),
        ));
    }
    fs::write(&path, code)
}

