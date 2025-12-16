# 交互式发布脚本
# 用法: 
#   .\tag-release.ps1           (运行后根据提示选择模式)
#   .\tag-release.ps1 v0.0.8    (带版本号运行，如果是测试模式会忽略版本号)

param(
    [Parameter(Mandatory=$false, Position=0)]
    [string]$Version
)

# 清屏并显示菜单
Clear-Host
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "       GitHub Actions 发布管理助手" -ForegroundColor Cyan
Write-Host "=========================================="
Write-Host ""
Write-Host "1. [测试构建] (Test Build)" -ForegroundColor Yellow
Write-Host "   - 动作: 仅推送代码到 main 分支"
Write-Host "   - 触发: CI 只进行编译测试，不发版"
Write-Host ""
Write-Host "2. [正式发布] (Official Release)" -ForegroundColor Green
Write-Host "   - 动作: 本地打 Tag -> 推送代码 -> 推送 Tag"
Write-Host "   - 触发: CI 编译、签名并上传 Release"
Write-Host ""
Write-Host "=========================================="

$selection = Read-Host "请选择模式 (输入 1 或 2)"

# ==========================================
# 模式 1: 测试构建 (仅推送代码)
# ==========================================
if ($selection -eq '1') {
    Write-Host "`n[测试模式] 正在推送 main 分支到远程..." -ForegroundColor Yellow
    
    # 直接推送代码
    git push github main
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[Error] 推送失败，请检查网络或 git 状态。" -ForegroundColor Red
        exit 1
    }

    Write-Host "[Success] 代码已推送，请前往 GitHub Actions 查看构建测试结果。" -ForegroundColor Green
}

# ==========================================
# 模式 2: 正式发布 (打 Tag 并推送)
# ==========================================
elseif ($selection -eq '2') {
    # 如果启动脚本时没带参数，现在要求输入版本号
    if ([string]::IsNullOrWhiteSpace($Version)) {
        Write-Host "`n请输入版本号 (例如 0.0.11):" -ForegroundColor Cyan
        $Version = Read-Host
    }

    # 简单的非空检查
    if ([string]::IsNullOrWhiteSpace($Version)) {
        Write-Host "[Error] 错误: 版本号不能为空！" -ForegroundColor Red
        exit 1
    }

    # 自动处理 v 前缀
    if (-not $Version.StartsWith("v")) {
        $Version = "v$Version"
    }

    Write-Host "`n[正式发布] 准备发布版本: $Version" -ForegroundColor Green

    # 1. 创建本地 tag
    Write-Host "Step 1: 创建本地 Tag $Version..." -ForegroundColor Cyan
    git tag $Version
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[Error] Tag 创建失败 (该 Tag 可能已存在?)" -ForegroundColor Red
        exit 1
    }

    # 2. 推送 main 分支 (确保代码是最新的)
    Write-Host "Step 2: 推送 main 分支到 github..." -ForegroundColor Cyan
    git push github main
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[Error] Main 分支推送失败" -ForegroundColor Red
        exit 1
    }

    # 3. 推送 tag
    Write-Host "Step 3: 推送 Tag $Version 到 github..." -ForegroundColor Cyan
    git push github $Version
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[Error] Tag 推送失败" -ForegroundColor Red
        exit 1
    }

    Write-Host "`n[Success] 成功! Tag $Version 已推送，GitHub Actions 将开始自动发布流程。" -ForegroundColor Green
}

# ==========================================
# 无效输入
# ==========================================
else {
    Write-Host "`n[Error] 无效的选择，脚本退出。" -ForegroundColor Red
    exit 1
}