# Rinko Backend

gRPC 后端服务，处理来自各个前端平台的消息和命令分发。

## 功能特性

- ✅ gRPC 服务端实现 `BotBackend` service
- ✅ 接收前端消息上报 (`report_message`)
- ✅ 命令分发到前端 (`subscribe_commands` - server streaming)
- ✅ 心跳健康检查 (`heartbeat`)
- ✅ 前端连接管理和状态跟踪
- ✅ 自动检测前端断线

## 运行

```bash
cd rinko-backend
cargo run --release
```

服务默认监听在 `0.0.0.0:50051`

## 配置文件

`config.toml`:
```toml
host = "0.0.0.0"
port = 50051
log_level = "info"
```

## API 示例

### 向前端发送命令

```rust
// 获取后端服务实例
let backend = BotBackendService::new();

// 发送命令到特定前端
let command = BotCommand {
    command_id: uuid::Uuid::new_v4().to_string(),
    command_type: "send_message".to_string(),
    parameters: [
        ("content".to_string(), "Hello!".to_string()),
        ("target_id".to_string(), "123456".to_string()),
    ].into(),
    timestamp: chrono::Utc::now().timestamp(),
};

backend.send_command_to_frontend("frontend-1", command).await?;
```

## 架构

```
rinko-backend/
├── src/
│   ├── main.rs          # 应用入口
│   ├── config.rs        # 配置加载
│   └── service.rs       # gRPC 服务实现
├── config.toml          # 配置文件
└── Cargo.toml
```
