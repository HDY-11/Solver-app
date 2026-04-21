import sys
import os
import ctypes
from ctypes import wintypes

def diagnose_python_environment():
    print("=== Python 环境诊断 ===")
    
    # 1. 解释器路径
    print(f"sys.executable: {sys.executable}")
    print(f"sys.base_exec_prefix: {sys.base_exec_prefix}")
    print(f"sys.prefix: {sys.prefix}")
    
    # 2. 标准库路径
    print(f"\n=== 标准库路径 ===")
    for path in sys.path:
        if 'Lib' in path or 'lib' in path:
            print(f"  {path}")
    
    # 3. DLL 信息（Windows）
    if os.name == 'nt':
        print(f"\n=== DLL 信息 ===")
        
        # 设置 kernel32 函数参数类型
        kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel32.GetModuleHandleW.argtypes = [wintypes.LPCWSTR]
        kernel32.GetModuleHandleW.restype = wintypes.HMODULE
        
        kernel32.GetModuleFileNameW.argtypes = [wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD]
        kernel32.GetModuleFileNameW.restype = wintypes.DWORD
        
        # 获取当前 Python 解释器使用的 DLL 名称
        # 方法1: 从 sys.version 获取版本号
        version = f"{sys.version_info.major}{sys.version_info.minor}"
        current_python_dll = f"python{version}.dll"
        
        # 方法2: 直接从当前进程的 executable 推断
        print(f"当前 Python DLL 应该是: {current_python_dll}")
        
        # 要检查的 DLL 名称列表
        dll_names = [current_python_dll, 'python313.dll', 'python312.dll', 'python311.dll']
        
        for dll_name in dll_names:
            try:
                handle = kernel32.GetModuleHandleW(dll_name)
                if handle:
                    print(f"✓ 找到 {dll_name} 句柄: {hex(handle)}")
                    
                    # 方法1: 标准方式获取路径
                    path_buffer = ctypes.create_unicode_buffer(260)
                    result = kernel32.GetModuleFileNameW(handle, path_buffer, 260)
                    
                    if result > 0:
                        print(f"  → {dll_name} 加载自: {path_buffer.value}")
                    else:
                        error_code = ctypes.get_last_error()
                        print(f"  ✗ 获取 {dll_name} 路径失败，错误码: {error_code}")
                        
                        # 方法2: 尝试更大的缓冲区
                        if error_code == 122:  # ERROR_INSUFFICIENT_BUFFER
                            path_buffer2 = ctypes.create_unicode_buffer(1024)
                            result2 = kernel32.GetModuleFileNameW(handle, path_buffer2, 1024)
                            if result2 > 0:
                                print(f"  → (长缓冲区) {dll_name}: {path_buffer2.value}")
                        
                        # 方法3: 使用 QueryFullProcessImageName 作为备选
                        try:
                            kernel32.QueryFullProcessImageNameW = kernel32.QueryFullProcessImageNameW
                            process_handle = kernel32.GetCurrentProcess()
                            buffer_size = wintypes.DWORD(260)
                            path_buffer3 = ctypes.create_unicode_buffer(260)
                            
                            if kernel32.QueryFullProcessImageNameW(process_handle, 0, path_buffer3, ctypes.byref(buffer_size)):
                                print(f"  → (进程名) 当前进程: {path_buffer3.value}")
                        except AttributeError:
                            pass
                else:
                    # DLL 未加载，尝试查找文件位置
                    print(f"✗ {dll_name} 未加载到当前进程")
                    
                    # 在常见路径中搜索 DLL
                    search_paths = [
                        sys.prefix,
                        sys.base_prefix,
                        os.path.dirname(sys.executable),
                        os.path.join(sys.prefix, 'DLLs'),
                        os.path.join(sys.base_prefix, 'DLLs'),
                        os.environ.get('PATH', '')
                    ]
                    
                    for search_path in search_paths:
                        if search_path and isinstance(search_path, str):
                            potential_path = os.path.join(search_path, dll_name)
                            if os.path.exists(potential_path):
                                print(f"  → 找到文件: {potential_path}")
                                break
                            
            except Exception as e:
                print(f"✗ {dll_name} 处理异常: {e}")
        
        # 4. 额外的进程模块枚举（可选）
        print(f"\n=== 已加载的 Python 相关模块 ===")
        try:
            # 使用 CreateToolhelp32Snapshot 枚举模块
            kernel32.CreateToolhelp32Snapshot = kernel32.CreateToolhelp32Snapshot
            kernel32.Module32First = kernel32.Module32First
            kernel32.Module32Next = kernel32.Module32Next
            kernel32.CloseHandle = kernel32.CloseHandle
            
            class MODULEENTRY32(ctypes.Structure):
                _fields_ = [
                    ("dwSize", wintypes.DWORD),
                    ("th32ModuleID", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD),
                    ("GlblcntUsage", wintypes.DWORD),
                    ("ProccntUsage", wintypes.DWORD),
                    ("modBaseAddr", ctypes.POINTER(ctypes.c_byte)),
                    ("modBaseSize", wintypes.DWORD),
                    ("hModule", wintypes.HMODULE),
                    ("szModule", ctypes.c_wchar * 256),
                    ("szExePath", ctypes.c_wchar * 260)
                ]
            
            TH32CS_SNAPMODULE = 0x00000008
            current_pid = kernel32.GetCurrentProcessId()
            
            snapshot = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, current_pid)
            if snapshot and snapshot != -1:
                module_entry = MODULEENTRY32()
                module_entry.dwSize = ctypes.sizeof(MODULEENTRY32)
                
                if kernel32.Module32First(snapshot, ctypes.byref(module_entry)):
                    while True:
                        if 'python' in module_entry.szModule.lower():
                            print(f"  {module_entry.szModule}: {module_entry.szExePath}")
                        if not kernel32.Module32Next(snapshot, ctypes.byref(module_entry)):
                            break
                kernel32.CloseHandle(snapshot)
        except (AttributeError, Exception) as e:
            print(f"  模块枚举失败: {e}")
    
    # 4. 虚拟环境标记
    print(f"\n=== 虚拟环境 ===")
    print(f"sys.prefix == sys.base_prefix: {sys.prefix == sys.base_prefix}")
    if hasattr(sys, 'real_prefix'):
        print(f"sys.real_prefix: {sys.real_prefix}")
    if hasattr(sys, 'base_prefix'):
        print(f"sys.base_prefix: {sys.base_prefix}")
    
    # 5. 关键模块导入测试
    print(f"\n=== 关键模块测试 ===")
    test_modules = ['os', 'json', 'numpy', 'matplotlib', 'pandas', 'torch', 'tensorflow']
    for module_name in test_modules:
        try:
            module = __import__(module_name)
            if hasattr(module, '__file__') and module.__file__:
                print(f"✓ {module_name}: {module.__file__}")
            else:
                print(f"✓ {module_name}: built-in")
        except ImportError as e:
            print(f"✗ {module_name}: {e}")
        except Exception as e:
            print(f"? {module_name}: 其他错误 - {e}")

if __name__ == "__main__":
    diagnose_python_environment()