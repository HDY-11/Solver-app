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

## 致谢

本项目使用了以下开源库：Tauri、React、Monaco Editor、PyO3、FontAwesome、SQLite 等。

## 许可

MIT
