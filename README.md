> **英文版在下面**，*English version below*
## 中文

# Solver

桌面分析工作台，基于 Tauri v2 + React + TypeScript。

## 功能

- **虚拟文件系统 (VFS)** — C 盘（SQLite + BlobStore）、B 盘（真实文件系统读写）、A 盘（导入只读）
- **Python 执行引擎** — 内嵌 Python 3.13，支持脚本编辑、运行、结果查看
- **多窗口分离/合并** — 标签拖拽分离为独立窗口，拖回 Nav 区域合并
- **Monaco 编辑器** — Python / JSON / Markdown / CSV 等语法高亮与查找
- **文件管理** — 新建、重命名、删除、拖拽移动、跨盘导入导出
- **运行结果管理** — 流式输出、历史记录、版本时间线

## 技术栈

| 层 | 技术 |
|----|------|
| 前端 | React 18, TypeScript, Monaco Editor, React Router v6 |
| 后端 | Rust (Tauri v2), PyO3 (Python 嵌入) |
| 存储 | SQLite, 自定义 BlobStore |
| 图标 | FontAwesome |
| 桌面 | Windows (WebView2), 自定义标题栏 |

## 开发

```bash
npm install
npm run tauri dev
```

## 构建

```bash
npm run tauri build
```

构建产物在 `src-tauri/target/release/bundle/`。

## 注意事项

- 项目根目录需要 `.venv` 文件夹（Python 3.13 虚拟环境），打包时会将 `Lib/site-packages/` 和标准库一起打包
- 修改 Python 版本需同步更新 `src-tauri/build.rs` 中的 `rustc-link-lib=python313`
- 本应用仍处于**验证原型**阶段，迭代调整较快，不代表最终质量和功能范围：
  - 为了加快开发速度，**一些**模块由ai写的**临时**逻辑拼凑且**未经仔细审查**，具体情况见**架构介绍**
  - 一些模块**未演进**出我设想的完整功能，**后续**会进行补全
  - 个人经验有限，望包容，提出您**宝贵的指导意见**(^.^)

## Solver-app 架构图

> 符号说明
> 
> 审：待审查
> 
> 改：待修改，可能发生较大变化
> 
> 扩：待扩展，接口不会发生较大变化

### 前端
- **api**
  - `config.ts` 应用配置读写(后端setting.toml) *审*
  - `console.ts` .cmdv console backend commands *审*
  - `events.ts` Tauri事件监听封装
  - `script.ts` 脚本统一操作封装 *改*
  - `vfs.ts` VFS操作统一封装 *扩*

- **commands**
  - `editorCommands.ts` 编辑器命令注册 *改*

- **components**
  - `ConfirmDialog.tsx` 确认对话框 *改*
  - `Loading.tsx` 加载与空状态组件 *审*
  - `NewScriptDialog.tsx` 新建文件对话框 *改*
  - `ResultDetail.tsx` 运行结果细节(废弃) *改*
  - `ResultHistoryItem.tsx` 运行结果条目 *改*
  - `RunList.tsx` 运行结果列表 *改*
  - `ShortcutHelp.tsx` 快捷键帮助面板 *改*
  - `TimelinePanel.tsx` 版本时间线面板 *改*
  - `Toast.tsx` 通知渲染组件

- **hooks**
  - `useSettings.tsx` 应用设置上下文 *改*
  - `useTabs.tsx` 标签页管理
  - `useToast.tsx` 全局Toast通知管理
  - `useWindow.tsx` 操作窗口管理(未使用) *扩*

- **layouts**
  - `Footer.tsx` 底部状态栏
  - `Header.tsx` 顶部导航栏
  - `Main.tsx` 主内容区
  - `Nav.tsx` 标签页导航栏
  - `NavBar.tsx` 左侧导航栏
  - `Sidebar.tsx` 侧边栏容器
  - `Toolbar.tsx` 渲染renderer的工具栏
  - `WelcomeView.tsx` 欢迎页

- **panels**
  - `SettingPanel.tsx` 应用设置面板 *扩*

- **registry**
  - `registry.ts` 渲染器注册 *审*
  - `type.ts` 类型注册 *改*

- **renderers**
  - `ConsoleRenderer.tsx` .cmdv 控制台文件渲染器 *审*
  - `HtmlViewer.tsx` HTML文件查看器
  - `PythonEditor.tsx` Python代码编辑器 *扩*
  - `RunResult.tsx` .run 文件(运行记录)文件渲染器 *扩*
  - `TextViewer.tsx` 文本文件编辑器
  - `useConsole.ts` .cmdv控制台状态管理Hook *审*

- **services**
  - `activeEditor.ts` 活跃编辑器引用 *改*
  - `commandService.ts` 命令系统 *改*

- **styles**

- **utils**
  - `icons.tsx` FontAwesome 图标统一映射

- `App.tsx` *改*

### 后端

- **Capabilities** / `default.json` Capability for all windows
- *plugin*
  - **window_enhance**
    - `lib.rs` WindowBehavior处理器签名 + BehaviorError?
    - `trait.rs` HookBehavior bitflags（兴趣声明）
    - `behavior.rs` HookBehavior 实现
    - `platform.rs` 平台抽象，目前: (Windows / no-op) *扩*
    - `window_proc.rs` 统一窗口进程 *改*
    - `manager.rs` WindowManager 门面，通过它注入Hook
    - `state.rs` WindowState 运行时状态
    - `command.rs` 公共API（不是Tauri命令）

- *rust-libs*
  - **env-system**
    - `lib.rs`
    - `config.rs` 提供应用配置 trait
    - `path.rs` 提供路径构造函数
    - `vfs_path.rs` 提供虚拟路径的辅助构造函数
  - **error-system**
    - `lib.rs` 定义AppError，提供`ResultExt`和`OptionExt` — 自动记录错误栈
  - **event-system**
    - `lib.rs` 包装Tauri事件的`emit!`, `emit_to!`, `listen`, `async_listen` 四个宏
  - **init-system**
    - `lib.rs` 管理初始化过程、上报进度
  - **log-system**
    - `lib.rs` 初始化逻辑
    - `handle.rs` 日志写入句柄，LogHandle，可从这里写入 手动日志区
    - `logger.rs` 实现`Log`，与`log`门面库接入，通过宏输出日志
    - `message.rs` 日志消息
    - `worker.rs` 工作线程
    - `rotating_file.rs` 轮换日志文件管理器
  - **lua-runtime**
    - `lib.rs` 混杂着的一团乱麻，需要整理 *改*
    - `vm.rs` 单个虚拟 *改*
  - **mem-buffer** *改*
    - `lib.rs` 环形内存缓冲区
  - **python-bridge**
    - `lib.rs` 混合的临时逻辑 *改*
    - `sdk.rs` 不该出现在这里 *改*
  - **utils**
    - `lib.rs` 提供了：栈上的环形缓冲区，句跨线程的所有权与计数
  - **vfs**
    - `lib.rs`
    - `pool.rs` 资源池
    - `query.rs` 管理与数据库
    - `real_fs.rs` A,B盘文件查找 *改*
    - `vfs_corers` 统一管理层
    - `vir_file.rs` VirFile 句柄

- *src*
  - `cli.rs` 命令行的注册和执行 *改*
  - `config.rs` 应用配置模块
  - `lib.rs` 各种业务逻辑的集中地，初始化、退出收尾的发生地 *改*

### 第三方库

- **后端**
  - **tauri** 2
  - **tauri-plugin-opener** 2
  - **tauri-plugin-dialog** 2
  - **tauri-plugin-log** 2
  - **serde** 1 features: derive
  - **serde_json** 1
  - **pyo3** 0.28.3 features: auto-initialize
  - **tokio** 1.52.1 features: time
  - **log** 0.4.29
  - **anyhow** 1.0.102
  - **windows** 0.62.2 features: Win32_UI_Input_KeyboardAndMouse
  - **crossbeam-channel** 0.5.15
  - **once_cell** 1.21.4
  - **thiserror** 2.0.18
  - **dirs** 6.0.0
  - **sha2** 0.11.0
  - **hex** 0.4.3
  - **uuid** 1 features: v4
  - **toml** 1.1.2
  - **r2d2** 0.8.10
  - **r2d2_sqlite** 0.34
  - **dashmap** 6.1.0
  - **tauri-build**

- **前端**
  - **@fortawesome/fontawesome-svg-core** 7.2.0
  - **@fortawesome/free-brands-svg-icons** 7.2.0
  - **@fortawesome/free-solid-svg-icons** 7.2.0
  - **@fortawesome/react-fontawesome** 0.3.1
  - **@monaco-editor/react** 4.7.0
  - **@tauri-apps/api** 2
  - **@tauri-apps/plugin-dialog** 2.7.0
  - **@tauri-apps/plugin-log** 2.8.0
  - **@tauri-apps/plugin-opener** 2
  - **@xterm/addon-fit** 0.11.0
  - **@xterm/addon-web-links** 0.12.0
  - **@xterm/xterm** 6.0.0
  - **react** 19.1.0
  - **react-dom** 19.1.0
  - **react-router-dom** 7.13.1


## 致谢

本项目使用了以下开源库：Tauri、React、Monaco Editor、PyO3、FontAwesome、SQLite 等。

## 许可

MIT


## English

# Solver

Desktop analysis workbench based on Tauri v2 + React + TypeScript.

## Features

- **Virtual File System (VFS)** — Drive C (SQLite + BlobStore), Drive B (native file system read/write), Drive A (import-only, read-only)
- **Python Execution Engine** — Embedded Python 3.13, supports script editing, execution, and result viewing
- **Multi-window Detach/Merge** — Drag tabs to detach as independent windows, drag back to Nav area to merge
- **Monaco Editor** — Syntax highlighting and search for Python / JSON / Markdown / CSV, etc.
- **File Management** — Create, rename, delete, drag-and-drop move, cross-drive import/export
- **Run Result Management** — Stream output, history records, version timeline

## Tech Stack

| Layer       | Technology                                      |
|-------------|-------------------------------------------------|
| Frontend    | React 18, TypeScript, Monaco Editor, React Router v6 |
| Backend     | Rust (Tauri v2), PyO3 (Python embedding)       |
| Storage     | SQLite, custom BlobStore                       |
| Icons       | FontAwesome                                     |
| Desktop     | Windows (WebView2), custom title bar           |

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Build artifacts are located in src-tauri/target/release/bundle/.

## Notes

- The project root requires a `.venv` folder (Python 3.13 virtual environment). During packaging, `Lib/site-packages/` and the standard library are bundled together.
- To change the Python version, update `rustc-link-lib=python313` in `src-tauri/build.rs` accordingly.
- This application is still in the **validation prototype** stage, with rapid iterations. It does not represent final quality or functionality scope:
  - To speed up development, **some** modules are assembled with **temporary** AI-generated logic and **not thoroughly reviewed**. See **Architecture Overview** for details.
  - Some modules **have not yet evolved** to my intended full functionality and will be **completed in future** iterations.
  - As a developer with limited experience, I appreciate your understanding and warmly welcome your **valuable feedback** (^.^)

## Solver-app Architecture Overview

> Legend
> 
> `[Review]` — Pending review
> 
> `[Modify]` — Pending modification, may change significantly
> 
> `[Extend]` — Pending extension, interface stable

### Frontend

- **api**
  - `config.ts` — App config read/write (backend setting.toml) `[Review]`
  - `console.ts` — .cmdv console backend commands `[Review]`
  - `events.ts` — Tauri event listener wrapper
  - `script.ts` — Script operation wrapper `[Modify]`
  - `vfs.ts` — VFS operation wrapper `[Extend]`

- **commands**
  - `editorCommands.ts` — Editor command registration `[Modify]`

- **components**
  - `ConfirmDialog.tsx` — Confirmation dialog `[Modify]`
  - `Loading.tsx` — Loading & empty state component `[Review]`
  - `NewScriptDialog.tsx` — New file dialog `[Modify]`
  - `ResultDetail.tsx` — Run result details (deprecated) `[Modify]`
  - `ResultHistoryItem.tsx` — Run result entry `[Modify]`
  - `RunList.tsx` — Run result list `[Modify]`
  - `ShortcutHelp.tsx` — Shortcut help panel `[Modify]`
  - `TimelinePanel.tsx` — Version timeline panel `[Modify]`
  - `Toast.tsx` — Toast notification component

- **hooks**
  - `useSettings.tsx` — App settings context `[Modify]`
  - `useTabs.tsx` — Tab management
  - `useToast.tsx` — Global Toast notification management
  - `useWindow.tsx` — Window operation management (unused) `[Extend]`

- **layouts**
  - `Footer.tsx` — Bottom status bar
  - `Header.tsx` — Top navigation bar
  - `Main.tsx` — Main content area
  - `Nav.tsx` — Tab navigation bar
  - `NavBar.tsx` — Left sidebar navigation
  - `Sidebar.tsx` — Sidebar container
  - `Toolbar.tsx` — Toolbar for renderer
  - `WelcomeView.tsx` — Welcome page

- **panels**
  - `SettingPanel.tsx` — App settings panel `[Extend]`

- **registry**
  - `registry.ts` — Renderer registry `[Review]`
  - `type.ts` — Type registry `[Modify]`

- **renderers**
  - `ConsoleRenderer.tsx` — .cmdv console file renderer `[Review]`
  - `HtmlViewer.tsx` — HTML file viewer
  - `PythonEditor.tsx` — Python code editor `[Extend]`
  - `RunResult.tsx` — .run file (execution record) renderer `[Extend]`
  - `TextViewer.tsx` — Text file editor
  - `useConsole.ts` — .cmdv console state management hook `[Review]`

- **services**
  - `activeEditor.ts` — Active editor reference `[Modify]`
  - `commandService.ts` — Command system `[Modify]`

- **styles**

- **utils**
  - `icons.tsx` — FontAwesome icon mapping

- `App.tsx` `[Modify]`

### Backend

- **Capabilities** / `default.json` — Capability for all windows
- *plugin*
  - **window_enhance**
    - `lib.rs` — WindowBehavior processor signature + BehaviorError
    - `trait.rs` — HookBehavior bitflags (interest declaration)
    - `behavior.rs` — HookBehavior implementation
    - `platform.rs` — Platform abstraction, currently: (Windows / no-op) `[Extend]`
    - `window_proc.rs` — Unified window procedure `[Modify]`
    - `manager.rs` — WindowManager facade, injects Hooks through it
    - `state.rs` — WindowState runtime state
    - `command.rs` — Public API (not Tauri commands)

- *rust-libs*
  - **env-system**
    - `lib.rs`
    - `config.rs` — Provides application configuration trait
    - `path.rs` — Provides path constructors
    - `vfs_path.rs` — Provides helper constructors for virtual paths
  - **error-system**
    - `lib.rs` — Defines AppError, provides `ResultExt` and `OptionExt` — automatically records error stack traces `[Review]`
  - **event-system**
    - `lib.rs` — Wraps Tauri events: four macros `emit!`, `emit_to!`, `listen`, `async_listen`
  - **init-system**
    - `lib.rs` — Manages initialization process, reports progress
  - **log-system**
    - `lib.rs` — Initialization logic
    - `handle.rs` — Log write handle (LogHandle), allows writing to manual log section
    - `logger.rs` — Implements `Log`, interfaces with `log` facade library, outputs logs via macros
    - `message.rs` — Log message
    - `worker.rs` — Worker thread
    - `rotating_file.rs` — Rotating log file manager
  - **lua-runtime**
    - `lib.rs` — Mixed messy code, needs refactoring `[Modify]`
    - `vm.rs` — Single VM instance `[Modify]`
  - **mem-buffer** `[Modify]`
    - `lib.rs` — Ring memory buffer
  - **python-bridge**
    - `lib.rs` — Messy temporary logic `[Modify]`
    - `sdk.rs` — Should not be here `[Modify]`
  - **utils**
    - `lib.rs` — Provides: stack-based ring buffer, cross-thread ownership and reference counting
  - **vfs**
    - `lib.rs`
    - `pool.rs` — Resource pool
    - `query.rs` — Management and database
    - `real_fs.rs` — Drive A, B file lookup `[Modify]`
    - `vfs_core.rs` — Unified management layer
    - `vir_file.rs` — VirFile handle

- *src*
  - `cli.rs` — Command line registration and execution `[Modify]`
  - `config.rs` — Application configuration module
  - `lib.rs` — Central hub for various business logic, where initialization and shutdown teardown occur `[Modify]`

### Third-Party Libraries

- **Backend**
  - **tauri** 2
  - **tauri-plugin-opener** 2
  - **tauri-plugin-dialog** 2
  - **tauri-plugin-log** 2
  - **serde** 1 features: derive
  - **serde_json** 1
  - **pyo3** 0.28.3 features: auto-initialize
  - **tokio** 1.52.1 features: time
  - **log** 0.4.29
  - **anyhow** 1.0.102
  - **windows** 0.62.2 features: Win32_UI_Input_KeyboardAndMouse
  - **crossbeam-channel** 0.5.15
  - **once_cell** 1.21.4
  - **thiserror** 2.0.18
  - **dirs** 6.0.0
  - **sha2** 0.11.0
  - **hex** 0.4.3
  - **uuid** 1 features: v4
  - **toml** 1.1.2
  - **r2d2** 0.8.10
  - **r2d2_sqlite** 0.34
  - **dashmap** 6.1.0
  - **tauri-build**

- **Frontend**
  - **@fortawesome/fontawesome-svg-core** 7.2.0
  - **@fortawesome/free-brands-svg-icons** 7.2.0
  - **@fortawesome/free-solid-svg-icons** 7.2.0
  - **@fortawesome/react-fontawesome** 0.3.1
  - **@monaco-editor/react** 4.7.0
  - **@tauri-apps/api** 2
  - **@tauri-apps/plugin-dialog** 2.7.0
  - **@tauri-apps/plugin-log** 2.8.0
  - **@tauri-apps/plugin-opener** 2
  - **@xterm/addon-fit** 0.11.0
  - **@xterm/addon-web-links** 0.12.0
  - **@xterm/xterm** 6.0.0
  - **react** 19.1.0
  - **react-dom** 19.1.0
  - **react-router-dom** 7.13.1


## Acknowledgements

This project uses the following open source libraries: Tauri, React, Monaco Editor, PyO3, FontAwesome, SQLite, and others.

## License

MIT
