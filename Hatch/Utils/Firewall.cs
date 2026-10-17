using WindowsFirewallHelper;
using WindowsFirewallHelper.FirewallRules;

namespace Hatch.Utils;

public static class Firewall
{
    private const string Hatch = Constants.ShortName;

    /// <summary>
    ///     需要入站防火墙规则的可执行文件白名单（相对于 bin 目录）
    /// </summary>
    private static readonly string[] AllowedInboundExecutables =
    {
        Constants.ExecutableName,
        "bin\\xray.exe",
        "bin\\sing-box.exe",
        "bin\\pcap2socks.exe"
    };

    /// <summary>
    ///     仅为白名单中的程序添加防火墙入站规则
    /// </summary>
    public static void AddHatchFwRules()
    {
        if (!FirewallWAS.IsLocallySupported)
        {
            Log.Warning("Windows Firewall Locally Unsupported");
            return;
        }

        try
        {
            var rule = FirewallManager.Instance.Rules.FirstOrDefault(r => r.Name == Hatch);
            if (rule != null)
            {
                if (RuleTargetsApplication(rule.ApplicationName))
                    return;

                RemoveHatchFwRules();
            }

            foreach (var relativePath in AllowedInboundExecutables)
            {
                var fullPath = Path.Combine(Global.HatchDir, relativePath);
                if (File.Exists(fullPath))
                    AddFwRule(Hatch, fullPath);
            }
        }
        catch (Exception e)
        {
            Log.Warning(e, "Create Hatch Firewall rules error");
        }
    }

    /// <summary>
    ///     清除防火墙规则 (Hatch 自带程序)
    /// </summary>
    public static void RemoveHatchFwRules()
    {
        if (!FirewallWAS.IsLocallySupported)
            return;

        try
        {
            foreach (var rule in FirewallManager.Instance.Rules.Where(r
                         => RuleTargetsApplication(r.ApplicationName) || r.Name == Hatch))
                FirewallManager.Instance.Rules.Remove(rule);
        }
        catch (Exception e)
        {
            Log.Warning(e, "Remove NPGA Firewall rules error");
        }
    }

    /// <summary>
    ///     判断规则指向的程序是否位于本程序目录下。Windows 路径不区分大小写，
    ///     且必须在目录分隔符处比较，否则 D:\NPGA-backup\NPGA.exe 会被误判为
    ///     属于 D:\NPGA。规则也可能没有应用程序名（端口规则），此时不应匹配。
    /// </summary>
    private static bool RuleTargetsApplication(string? applicationName)
    {
        if (string.IsNullOrWhiteSpace(applicationName))
            return false;

        try
        {
            var applicationDirectory = Path.GetFullPath(Global.HatchDir).TrimEnd(Path.DirectorySeparatorChar)
                                       + Path.DirectorySeparatorChar;
            return Path.GetFullPath(applicationName)
                .StartsWith(applicationDirectory, StringComparison.OrdinalIgnoreCase);
        }
        catch (ArgumentException)
        {
            // A rule with a malformed path cannot belong to this installation.
            return false;
        }
    }

    #region 封装

    private static void AddFwRule(string ruleName, string exeFullPath)
    {
        var rule = new FirewallWASRule(ruleName,
            exeFullPath,
            FirewallAction.Allow,
            FirewallDirection.Inbound,
            FirewallProfiles.Private | FirewallProfiles.Public | FirewallProfiles.Domain);

        FirewallManager.Instance.Rules.Add(rule);
    }

    #endregion
}
