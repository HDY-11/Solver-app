use error_system::AppError;
use error_system::ResultLogExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::*;
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::env::current_exe;
use std::env::set_var;
use std::ffi::CString;
use std::fs;
use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::path::{Path, PathBuf};
use tokio::task::spawn_blocking;

// ==================== 路径获取 ====================

pub fn get_resource_root() -> PathBuf {
    let exe_dir = current_exe()
        .expect("无法获取可执行文件所在路径")
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();

    #[cfg(debug_assertions)]
    {
        let mut root = exe_dir.clone();
        while !root.join("src-tauri").exists() {
            root = root.parent().expect("无法找到项目根目录").to_path_buf();
        }
        root
    }

    #[cfg(not(debug_assertions))]
    {
        exe_dir
    }
}

pub fn get_python_home_path() -> PathBuf {
    get_resource_root().join("Python")
}

pub fn get_scripts_path() -> PathBuf {
    get_resource_root().join("scripts")
}

pub fn get_site_packages_path() -> PathBuf {
    get_python_home_path().join("Lib").join("site-packages")
}

pub fn init_python_venv(py: Python) -> PyResult<()> {
    #[cfg(not(debug_assertions))]
    unsafe {
        let python_home = get_python_home_path();
        let python_home_str = python_home
            .to_str()
            .ok_or_else(|| PyValueError::new_err("python 路径包含非 UTF-8 字符"))
            .inspect_log("解析 PYTHONHOME 路径失败")?;
        set_var("PYTHONHOME", python_home_str);
    }

    let site_packages = get_site_packages_path();
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

pub async fn run_script(script_path: String) -> Result<String, PyErr> {
    let result = spawn_blocking(move || -> PyResult<String> {
        Python::attach(|py| {
            init_python_venv(py)?;

            let script_content = fs::read_to_string(&script_path)
                .inspect_log(format!("读取脚本文件: {}", script_path))?;

            let script_cstr = CString::new(script_content).inspect_log("脚本内容包含空字节")?;

            let sys = py.import("sys").inspect_log("导入 sys 模块失败")?;
            let io = py.import("io").inspect_log("导入 io 模块失败")?;

            let stdout = io
                .call_method0("StringIO")
                .inspect_log("创建 StringIO (stdout) 失败")?;
            let stderr = io
                .call_method0("StringIO")
                .inspect_log("创建 StringIO (stderr) 失败")?;

            let old_stdout = sys.getattr("stdout").inspect_log("获取原 stdout 失败")?;
            let old_stderr = sys.getattr("stderr").inspect_log("获取原 stderr 失败")?;

            sys.setattr("stdout", &stdout)
                .inspect_log("重定向 stdout 失败")?;
            sys.setattr("stderr", &stderr)
                .inspect_log("重定向 stderr 失败")?;

            let run_result = py.run(&script_cstr, None, None);

            let stdout_str = stdout
                .call_method0("getvalue")
                .inspect_log("获取 stdout 内容失败")?
                .extract()
                .inspect_log("解析 stdout 内容失败")?;
            let stderr_str: String = stderr
                .call_method0("getvalue")
                .inspect_log("获取 stderr 内容失败")?
                .extract()
                .inspect_log("解析 stderr 内容失败")?;

            // 恢复原 stdout/stderr，忽略错误（尽力而为）
            let _ = sys.setattr("stdout", old_stdout);
            let _ = sys.setattr("stderr", old_stderr);

            match run_result {
                Ok(_) => Ok(stdout_str),
                Err(e) => {
                    let msg = format!("{}\n{}", stderr_str, e);
                    Err(PyRuntimeError::new_err(msg))
                }
            }
        })
    })
    .await
    .expect_log("py脚本运行失败");
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
