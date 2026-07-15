using WindowsFirewallHelper;
using WindowsFirewallHelper.FirewallRules;

namespace Hatch.Utils;

public static class Firewall
{
    private const string Hatch = "Hatch";

    /// <summary>
    ///     需要入站防火墙规则的可执行文件白名单（相对于 bin 目录）
    /// </summary>
    private static readonly string[] AllowedInboundExecutables =
    {
        "Hatch.exe",
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
                if (rule.ApplicationName.StartsWith(Global.HatchDir))
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
                         => r.ApplicationName?.StartsWith(Global.HatchDir, StringComparison.OrdinalIgnoreCase) ?? r.Name == Hatch))
                FirewallManager.Instance.Rules.Remove(rule);
        }
        catch (Exception e)
        {
            Log.Warning(e, "Remove Hatch Firewall rules error");
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
