# Hatch - 使用说明 | User Guide

[English](#english) | [中文](#中文)

---

## English

### Quick Start

1. **Run the Application**
   - Double-click `Hatch.exe` to start

2. **Add a Server**
   - Click the server dropdown menu
   - Select "Add Server" or import from subscription
   - Fill in server details and save

3. **Select a Mode**
   - Choose a mode from the mode dropdown
   - Recommended: "Bypass LAN" for general use

4. **Start Connection**
   - Click the "Start" button
   - Wait for connection to establish
   - Check the status indicator

### Important Notes

- **First Run**: The application may take a few seconds to initialize
- **Firewall**: Allow Hatch through Windows Firewall when prompted
- **Administrator**: Some modes (TUN/TAP) require administrator privileges
- **Antivirus**: Add Hatch to your antivirus whitelist if needed

### Folder Structure

```
Hatch/
├── Hatch.exe          # Main application
├── bin/               # Core components and dependencies
│   ├── xray.exe       # Xray-core proxy engine
│   ├── aiodns.bin     # DNS resolver
│   ├── Redirector.bin # Network redirector
│   └── ...
├── mode/              # Routing mode configurations
├── i18n/              # Language files
└── data/              # User data (created on first run)
    ├── servers.json   # Server configurations
    └── settings.json  # Application settings
```

### Troubleshooting

**Connection Failed**
- Check server configuration
- Verify network connectivity
- Try a different mode

**High Latency**
- Test server latency (right-click server)
- Switch to a closer server
- Check local network conditions

**Application Won't Start**
- Run as administrator
- Check Windows Firewall settings
- Verify .NET runtime is installed

### Support

- GitHub Issues: https://github.com/OffroadOps/Hatch/issues
- Documentation: https://github.com/OffroadOps/Hatch

---

## 中文

### 快速开始

1. **运行应用程序**
   - 双击 `Hatch.exe` 启动

2. **添加服务器**
   - 点击服务器下拉菜单
   - 选择"添加服务器"或从订阅导入
   - 填写服务器详情并保存

3. **选择模式**
   - 从模式下拉菜单选择一个模式
   - 推荐：一般使用选择 "Bypass LAN"

4. **开始连接**
   - 点击"启动"按钮
   - 等待连接建立
   - 查看状态指示器

### 重要提示

- **首次运行**: 应用程序可能需要几秒钟初始化
- **防火墙**: 提示时允许 Hatch 通过 Windows 防火墙
- **管理员权限**: 某些模式（TUN/TAP）需要管理员权限
- **杀毒软件**: 如需要，将 Hatch 添加到杀毒软件白名单

### 文件夹结构

```
Hatch/
├── Hatch.exe          # 主程序
├── bin/               # 核心组件和依赖项
│   ├── xray.exe       # Xray-core 代理引擎
│   ├── aiodns.bin     # DNS 解析器
│   ├── Redirector.bin # 网络重定向器
│   └── ...
├── mode/              # 路由模式配置
├── i18n/              # 语言文件
└── data/              # 用户数据（首次运行时创建）
    ├── servers.json   # 服务器配置
    └── settings.json  # 应用程序设置
```

### 故障排除

**连接失败**
- 检查服务器配置
- 验证网络连接
- 尝试不同的模式

**延迟高**
- 测试服务器延迟（右键点击服务器）
- 切换到更近的服务器
- 检查本地网络状况

**应用程序无法启动**
- 以管理员身份运行
- 检查 Windows 防火墙设置
- 验证是否安装了 .NET 运行时

### 支持

- GitHub Issues: https://github.com/OffroadOps/Hatch/issues
- 文档: https://github.com/OffroadOps/Hatch

---

**版本 | Version**: 2.0.0  
**基于 | Based on**: Netch 1.9.7  
**许可证 | License**: GPL-3.0
