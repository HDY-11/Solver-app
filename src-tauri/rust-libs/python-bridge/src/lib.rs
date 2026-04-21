use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::exceptions::{PyIOError, PyValueError, PyRuntimeError};
use std::fs;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::env::current_exe;
use std::io::{Result as IoResult, Error as IoError, ErrorKind};
use tokio::task::spawn_blocking;
use std::env::set_var;


// ==================== 路径获取（编译时决定策略） ====================

/// 获取资源根目录
/// - debug_assertions（开发环境）：返回项目根目录
/// - release（生产环境）：返回可执行文件所在目录
pub fn get_resource_root() -> PathBuf {
    let exe_dir = current_exe()
        .expect("无法获取可执行文件路径")
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();

    #[cfg(debug_assertions)]
    {
        // 开发环境：向上找到包含 src-tauri 的目录（项目根目录）
        let mut root = exe_dir.clone();
        while !root.join("src-tauri").exists() {
            root = root
                .parent()
                .expect("无法找到项目根目录（缺少 src-tauri）")
                .to_path_buf();
        }
        root
    }

    #[cfg(not(debug_assertions))]
    {
        // 生产环境：资源直接放在可执行文件同级
        exe_dir
    }
}

/// 获取 .venv 目录路径
pub fn get_venv_path() -> PathBuf {
    get_resource_root().join(".venv")
}

/// 获取 scripts 目录路径
pub fn get_scripts_path() -> PathBuf {
    get_resource_root().join("scripts")
}

/// 获取 site-packages 路径
pub fn get_site_packages_path() -> PathBuf {
    get_venv_path().join("Lib").join("site-packages")
}

/// 初始化 Python 环境，设置运行时路径
/// 使用条件编译：开发环境直接用系统探测，生产环境强制设置 PYTHONHOME
pub fn init_python_venv(py: Python) -> PyResult<()> {
    #[cfg(not(debug_assertions))]
    unsafe {
        // 生产环境：强制设置 PYTHONHOME 为打包的 .venv 目录
        let venv_root = get_venv_path();
        let venv_root_str = venv_root
            .to_str()
            .ok_or_else(|| PyErr::new::<PyValueError, _>("venv 路径包含非 UTF-8 字符"))?;
        
        set_var("PYTHONHOME", venv_root_str);
    }

    // 开发环境和生产环境都需要：将 site-packages 和 scripts 加入 sys.path
    let site_packages = get_site_packages_path();
    let scripts_path = get_scripts_path();

    let sys = py.import("sys")?;

    let binding = sys.getattr("path")?;
    let sys_path: &Bound<'_, PyList> = binding.downcast()?;

    let site_packages_str = site_packages
        .to_str()
        .ok_or_else(|| PyErr::new::<PyValueError, _>("site-packages 路径包含非 UTF-8 字符"))?;
    let scripts_path_str = scripts_path
        .to_str()
        .ok_or_else(|| PyErr::new::<PyValueError, _>("scripts 路径包含非 UTF-8 字符"))?;

    sys_path.insert(0, site_packages_str)?;
    sys_path.insert(0, scripts_path_str)?;

    Ok(())
}

/// 执行 Python 脚本
pub async fn run_script(script_path: String) -> PyResult<String> {
    let handle = spawn_blocking(move || {
        Python::attach(|py| {
            init_python_venv(py)?;

            let script_content = fs::read_to_string(&script_path)
                .map_err(|e| PyErr::new::<PyIOError, _>(
                    format!("无法读取脚本 {}: {}", script_path, e)
                ))?;

            let script_content = CString::new(script_content)
                .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))?;

            // 捕获 stdout/stderr
            let sys = py.import("sys")?;
            let io = py.import("io")?;
            let stdout = io.call_method0("StringIO")?;
            let stderr = io.call_method0("StringIO")?;
            
            let old_stdout = sys.getattr("stdout")?;
            let old_stderr = sys.getattr("stderr")?;
            
            sys.setattr("stdout", stdout.clone())?;
            sys.setattr("stderr", stderr.clone())?;

            let result = py.run(&script_content, None, None);

            let stdout_str: String = stdout.call_method0("getvalue")?.extract()?;
            let stderr_str: String = stderr.call_method0("getvalue")?.extract()?;

            sys.setattr("stdout", old_stdout)?;
            sys.setattr("stderr", old_stderr)?;

            match result {
                Ok(_) => Ok(stdout_str),
                Err(e) => {
                    let error_msg = format!("{}\n{}", stderr_str, e);
                    Err(PyErr::new::<PyRuntimeError, _>(error_msg))
                }
            }
        })
    });

    handle
        .await
        .map_err(|e| PyErr::new::<PyRuntimeError, _>(format!("阻塞任务执行失败: {}", e)))?
}

/// 保存脚本
pub fn save_script(code: String, file_path: String) -> IoResult<()> {
    let path = Path::new(&file_path);
    let parent = path.parent().ok_or_else(|| {
        IoError::new(ErrorKind::NotFound, "无法获取父目录")
    })?;
    
    if !parent.exists() {
        return Err(IoError::new(ErrorKind::NotFound, format!("目录不存在: {}", parent.display())));
    }

    fs::write(path, code)
}