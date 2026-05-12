use error_system::ResultLogExt;
use env_system::paths::{exe_root, embedded_site_packages};
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::*;
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::ffi::CString;
use std::fs;
use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::path::{Path, PathBuf};
use tokio::task::spawn_blocking;

// ==================== (临时)路径获取 ====================
pub fn get_scripts_path() -> PathBuf {
    exe_root().join("scripts")
}

pub fn init_python_venv(py: Python) -> PyResult<()> {
    let site_packages = embedded_site_packages();
    let scripts_path = get_scripts_path();

    let sys = py.import("sys").inspect_log("导入 sys 模块失败")?;
    let binding = sys.getattr("path").inspect_log("获取 sys.path 失败")?;
    let sys_path: &Bound<'_, PyList> = binding.cast()?;

    let site_packages_str = site_packages
        .to_str()
        .ok_or_else(|| PyValueError::new_err("site-packages 路径包含非 UTF-8 字符"))
        .inspect_log("site-packages 路径非法")?;
    let scripts_path_str = scripts_path
        .to_str()
        .ok_or_else(|| PyValueError::new_err("scripts 路径包含非 UTF-8 字符"))
        .inspect_log("scripts 路径非法")?;

    sys_path
        .insert(0, site_packages_str)
        .inspect_log("将 site-packages 插入 sys.path 失败")?;
    sys_path
        .insert(0, scripts_path_str)
        .inspect_log("将 scripts 插入 sys.path 失败")?;

    Ok(())
}

pub async fn run_script(script_path: &str) -> PyResult<(String, String)> {
    let script_path = script_path.to_owned();
    let result = spawn_blocking(move || -> PyResult<(String, String)> {
        Python::attach(|py| {
            init_python_venv(py)?;

            let script_content = std::fs::read_to_string(&script_path)
                .map_err(|e| PyErr::new::<PyRuntimeError, _>(e.to_string()))?;
            let script_content = CString::new(script_content).expect_log("转换成CString失败");
            let sys = py.import("sys")?;
            let io = py.import("io")?;

            let stdout = io.call_method0("StringIO")?;
            let stderr = io.call_method0("StringIO")?;

            let old_stdout = sys.getattr("stdout")?;
            let old_stderr = sys.getattr("stderr")?;

            sys.setattr("stdout", &stdout)?;
            sys.setattr("stderr", &stderr)?;

            let _ = py.run(&script_content, None, None);

            let _ = sys.setattr("stdout", old_stdout);
            let _ = sys.setattr("stderr", old_stderr);

            let stdout_str: String = stdout.call_method0("getvalue")?.extract()?;
            let stderr_str: String = stderr.call_method0("getvalue")?.extract()?;

            Ok((stdout_str, stderr_str))
        })
    })
    .await
    .map_err(|e| PyErr::new::<PyRuntimeError, _>(e.to_string()))?;
    
    result
}

pub fn save_script(code: String, file_path: String) -> IoResult<()> {
    let path = Path::new(&file_path);
    let parent = path
        .parent()
        .ok_or_else(|| IoError::new(ErrorKind::NotFound, "无法获取父目录"))?;

    if !parent.exists() {
        return Err(IoError::new(
            ErrorKind::NotFound,
            format!("目录不存在: {}", parent.display()),
        ));
    }

    fs::write(path, code).inspect_log(format!("保存脚本失败: {}", file_path))?;

    Ok(())
}
