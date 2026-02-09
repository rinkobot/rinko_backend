# Rinko - Multi-Platform Bot Framework

Cargo Workspace 项目，包含前后端通信的完整解决方案。

## 项目结构

```
rinko/
├── Cargo.toml              # Workspace 配置
├── proto/                  # Protocol Buffers 定义
│   └── bot.proto
├── rinko-common/           # 共享库
│   ├── src/
│   │   ├── proto/          # 生成的 proto 代码
│   │   ├── types.rs        # 共享类型 (Platform)
│   │   └── lib.rs
│   └── build.rs            # Proto 编译脚本
├── rinko-frontend/         # 前端服务
│   ├── src/
│   │   ├── frontend/       # QQ/Telegram 等平台接入
│   │   ├── backend/        # gRPC 客户端
│   │   ├── config.rs
│   │   └── main.rs
│   └── config.toml
└── rinko-backend/          # 后端服务
    ├── src/
    │   ├── service.rs      # gRPC 服务端
    │   ├── config.rs
    │   └── main.rs
    └── config.toml
```

## 快速开始

### 1. 构建整个 workspace

```bash
cd rinko
cargo build --workspace --release
```

### 2. 分别编译

```bash
# 只编译前端
cargo build -p rinko-frontend --release

# 只编译后端
cargo build -p rinko-backend --release
```

### 3. 运行服务

**启动后端（gRPC 服务器）：**
```bash
cd rinko-backend
cargo run --release
```

**启动前端（QQ Bot Webhook）：**
```bash
cd rinko-frontend
cargo run --release
```

## 架构特点

✅ **共享类型定义** - `rinko-common` 提供统一的 proto 类型  
✅ **独立部署** - 前后端可分别编译和部署  
✅ **类型安全** - Proto 定义自动同步，编译时保证类型一致  
✅ **自动重连** - 前端支持运行时连接/断线重连  
✅ **gRPC 通信** - 高性能、跨语言的 RPC 框架  

## 依赖管理

所有版本在 workspace `Cargo.toml` 中统一管理：

```toml
[workspace.dependencies]
tokio = { version = "1.49.0", features = ["full"] }
tonic = "0.12.3"
# ...
```

子项目中使用：
```toml
[dependencies]
tokio = { workspace = true }
rinko-common = { path = "../rinko-common" }
```

## 开发指南

### 修改 Proto 定义

1. 编辑 `proto/bot.proto`
2. 运行 `cargo build -p rinko-common` 生成新代码
3. 前后端自动使用最新定义

### 添加新平台

1. 在 `proto/bot.proto` 添加 Platform 枚举值
2. 在 `rinko-common/src/types.rs` 更新转换逻辑
3. 在 `rinko-frontend/src/frontend/` 添加新平台模块

### 测试

```bash
# 测试所有项目
cargo test --workspace

# 测试特定项目
cargo test -p rinko-backend
```
