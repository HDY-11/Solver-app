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
> 审：待审查
> 改：待修改，可能发生较大变化
> 扩：待扩展，接口不会发生较大变化

### 前端
- **api**
- config.ts 应用配置读写(后端setting.toml) *审*
- console.ts .cmdv console backend commands *审*
- events.ts Tauri事件监听封装
- script.ts 脚本统一操作封装 *改*
- vfs.ts VFS操作统一封装 *扩*

- **commands**
  - editorCommands.ts 编辑器命令注册 *改*

- **components**
  - ConfirmDialog.tsx 确认对话框 *改*
  - Loading.tsx 加载与空状态组件 *审*
  - NewScriptDialog.tsx 新建文件对话框 *改*
  - ResultDetail.tsx 运行结果细节(废弃) *改*
  - ResultHistoryItem.tsx 运行结果条目 *改*
  - RunList.tsx 运行结果列表 *改*
  - ShortcutHelp.tsx 快捷键帮助面板 *改*
  - TimelinePanel.tsx 版本时间线面板 *改*
  - Toast.tsx 通知渲染组件

- **hooks**
  - useSettings.tsx 应用设置上下文 *改*
  - useTabs.tsx 标签页管理
  - useToast.tsx 全局Toast通知管理
  - useWindow.tsx 操作窗口管理(未使用) *扩*

- **layouts**
  - Footer.tsx 底部状态栏
  - Header.tsx 顶部导航栏
  - Main.tsx 主内容区
  - Nav.tsx 标签页导航栏
  - NavBar.tsx 左侧导航栏
  - Sidebar.tsx 侧边栏容器
  - Toolbar.tsx 渲染renderer的工具栏
  - WelcomeView.tsx 欢迎页

- **panels**
  - SettingPanel.tsx 应用设置面板 *扩*

- **registry**
  - registry.ts 渲染器注册 *审*
  - type.ts 类型注册 *改*

- **renderers**
  - ConsoleRenderer.tsx .cmdv 控制台文件渲染器 *审*
  - HtmlViewer.tsx HTML文件查看器
  - PythonEditor.tsx Python代码编辑器 *扩*
  - RunResult.tsx .run 文件(运行记录)文件渲染器 *扩*
  - TextViewer.tsx 文本文件编辑器
  - useConsole.ts .cmdv控制台状态管理Hook *审*

- **services**
  - activeEditor.ts 活跃编辑器引用 *改*
  - commandService.ts 命令系统 *改*

- **styles**

- **utils**
  - icons.tsx FontAwesome 图标统一映射

- App.tsx *改*

### 后端
> 此项待添加

## 致谢

本项目使用了以下开源库：Tauri、React、Monaco Editor、PyO3、FontAwesome、SQLite 等。

## 许可

MIT
