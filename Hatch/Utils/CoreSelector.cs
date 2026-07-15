using Hatch.Models;
using Hatch.Servers;
using Hatch.Servers.Hysteria2;

namespace Hatch.Utils;

public static class CoreSelector
{
    public static CoreType SelectCoreForServer(Server server)
    {
        return server switch
        {
            Hysteria2Server => CoreType.SingBox,
            VLESSServer => CoreType.XrayCore,
            VMessServer => CoreType.XrayCore,
            TrojanServer => CoreType.XrayCore,
            ShadowsocksServer => CoreType.XrayCore,
            ShadowsocksRServer => CoreType.XrayCore,
            Socks5Server => CoreType.XrayCore,
            WireGuardServer => CoreType.XrayCore,
            _ => CoreType.XrayCore
        };
    }

    public static bool IsCoreAvailable(CoreType coreType)
    {
        return coreType switch
        {
            CoreType.XrayCore => File.Exists(Path.Combine("bin", "xray.exe")),
            CoreType.SingBox => File.Exists(Path.Combine("bin", "sing-box.exe")),
            _ => false
        };
    }

    public static string GetCoreExecutableName(CoreType coreType)
    {
        return coreType switch
        {
            CoreType.XrayCore => "xray.exe",
            CoreType.SingBox => "sing-box.exe",
            _ => throw new NotSupportedException($"Unknown core type: {coreType}")
        };
    }

    public static string GetCoreDisplayName(CoreType coreType)
    {
        return coreType switch
        {
            CoreType.XrayCore => "Xray",
            CoreType.SingBox => "sing",
            _ => "Unknown"
        };
    }

    public static string GetCoreFullName(CoreType coreType)
    {
        return coreType switch
        {
            CoreType.XrayCore => "Xray-core",
            CoreType.SingBox => "sing-box",
            _ => "Unknown"
        };
    }

    public static List<CoreType> GetSupportedCores(Server server)
    {
        var cores = new List<CoreType>();

        if (SupportsXrayCore(server) && IsCoreAvailable(CoreType.XrayCore))
            cores.Add(CoreType.XrayCore);

        if (SupportsSingBox(server) && IsCoreAvailable(CoreType.SingBox))
            cores.Add(CoreType.SingBox);

        return cores;
    }

    private static bool SupportsXrayCore(Server server)
    {
        return server is VLESSServer or VMessServer or TrojanServer
            or ShadowsocksServer or ShadowsocksRServer or Socks5Server
            or WireGuardServer;
    }

    private static bool SupportsSingBox(Server server)
    {
        return server is Hysteria2Server;
    }

    public static string GetProtocolSupportInfo(Server server)
    {
        var supportedCores = GetSupportedCores(server);

        if (!supportedCores.Any())
        {
            var requiredCore = SelectCoreForServer(server);
            return $"Requires {GetCoreFullName(requiredCore)}";
        }

        var coreNames = string.Join(", ", supportedCores.Select(GetCoreFullName));
        return $"Supported by {coreNames}";
    }

    public static Dictionary<CoreType, bool> CheckAllCoresAvailability()
    {
        return new Dictionary<CoreType, bool>
        {
            { CoreType.XrayCore, IsCoreAvailable(CoreType.XrayCore) },
            { CoreType.SingBox, IsCoreAvailable(CoreType.SingBox) }
        };
    }

    public static Dictionary<CoreType, string> GetMissingCores()
    {
        var missing = new Dictionary<CoreType, string>();

        if (!IsCoreAvailable(CoreType.XrayCore))
            missing.Add(CoreType.XrayCore, "VLESS, VMess, Trojan, Shadowsocks, ShadowsocksR, SOCKS, WireGuard");

        if (!IsCoreAvailable(CoreType.SingBox))
            missing.Add(CoreType.SingBox, "Hysteria2");

        return missing;
    }
}
