using Hatch.Models;
using Hatch.Servers.Hysteria2;

namespace Hatch.Servers.SingBox;

/// <summary>
/// sing-box 配置文件生成工具。
/// </summary>
public static class SingBoxConfigUtils
{
    /// <summary>
    /// 鐢熸垚 sing-box 瀹㈡埛绔厤缃?
    /// </summary>
    public static async Task<object> GenerateClientConfigAsync(Server server)
    {
        // 鍩虹閰嶇疆缁撴瀯
        var config = new
        {
            log = new
            {
                level = "info",
                timestamp = true
            },
            inbounds = new[]
            {
                new
                {
                    type = "mixed",
                    tag = "mixed-in",
                    listen = Global.Settings.LocalAddress,
                    listen_port = Global.Settings.Socks5LocalPort,
                    sniff = true,
                    sniff_override_destination = false
                }
            },
            outbounds = new[]
            {
                await GenerateOutboundAsync(server),
                new
                {
                    type = "direct",
                    tag = "direct"
                },
                new
                {
                    type = "block",
                    tag = "block"
                }
            }
        };

        return await Task.FromResult(config);
    }

    /// <summary>
    /// 鏍规嵁鏈嶅姟鍣ㄧ被鍨嬬敓鎴愬搴旂殑 outbound 閰嶇疆
    /// </summary>
    private static async Task<object> GenerateOutboundAsync(Server server)
    {
        return server switch
        {
            Hysteria2Server hy2 => await GenerateHysteria2OutboundAsync(hy2),
            // TUIC and ShadowTLS can be added here when their server models are implemented.
            _ => throw new NotSupportedException($"sing-box does not support {server.Type} protocol")
        };
    }

    /// <summary>
    /// 鐢熸垚 Hysteria2 outbound 閰嶇疆
    /// </summary>
    private static async Task<object> GenerateHysteria2OutboundAsync(Hysteria2Server server)
    {
        var config = new
        {
            type = "hysteria2",
            tag = "proxy",
            server = server.Hostname,
            server_port = server.Port,
            password = server.Password,

            // 甯﹀閰嶇疆
            up_mbps = server.UploadBandwidth ?? Global.Settings.CoreConfig.SingBox.Hysteria2UpMbps,
            down_mbps = server.DownloadBandwidth ?? Global.Settings.CoreConfig.SingBox.Hysteria2DownMbps,

            // 混淆配置
            obfs = string.IsNullOrEmpty(server.ObfuscationType) ? null : new
            {
                type = server.ObfuscationType,
                password = server.ObfuscationPassword
            },

            // TLS 閰嶇疆
            tls = new
            {
                enabled = true,
                server_name = server.ServerName ?? server.Hostname,
                insecure = server.TLSInsecure,
                alpn = server.ALPN ?? new[] { "h3" }
            }
        };

        return await Task.FromResult(config);
    }

}
