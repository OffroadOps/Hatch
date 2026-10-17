# NPGA 重构工作区

本目录是重构（详见仓库根目录 `REFACTORING_PROPOSAL.md`）的工作区：
计划、决策记录、验证资产都放在这里，不属于产品代码。

> 决策基线：`DECISIONS.md`（ADR 格式，新决策必须追加，不改旧条目）。

## 目录结构

```
refactor/
├── README.md                 本文件
├── DECISIONS.md              架构决策记录（ADR）
├── PHASE-1-CHECKLIST.md      第一阶段任务清单与进度
└── verify/
    └── tun-process/          sing-box TUN + process_name 替代 Redirector 的验证资产
        ├── README.md         验证步骤与通过标准
        ├── tun-process.json  最小可验证配置模板
        └── verify.ps1        自动化验证脚本（管理员运行）
```

## 当前阶段

第一阶段：收敛与验证。C# 版已冻结，Rust 版为唯一主线。
进行中的工作见 `PHASE-1-CHECKLIST.md`。

## 与主仓库的关系

- `npga-rust/` 是唯一活跃的产品代码树。
- `Hatch/`（C#）已冻结归档，仅接受安全问题修复，不再合入新功能。
- 本工作区不参与构建，`build-rust.ps1` 不会引用它（原 `build.ps1` 已随归档移出，见 ADR-007）。
