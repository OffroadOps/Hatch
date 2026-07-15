using System.Net;
using System.Text.Json;
using Hatch.Controllers;
using Hatch.Interfaces;
using Hatch.Models;
using Hatch.Servers;

namespace Hatch.Servers.SingBox;

/// <summary>
/// sing-box 鎺у埗鍣?
/// 用于 Hysteria2、TUIC、ShadowTLS、WireGuard 等协议。
/// </summary>
public class SingBoxController : Guard, IServerController
{
    public SingBoxController() : base("sing-box.exe")
    {
    }

    protected override IEnumerable<string> StartedKeywords => new[] { "started" };

    protected override IEnumerable<string> FailedKeywords => new[] { "failed to", "Failed to", "error" };

    public override string Name => "sing-box";

    public ushort? Socks5LocalPort { get; set; }

    public string? LocalAddress { get; set; }

    public virtual async Task<Socks5Server> StartAsync(Server s)
    {
        // 生成 sing-box 配置文件。
        await using (var fileStream = new FileStream(Constants.TempConfig, FileMode.Create, FileAccess.Write, FileShare.Read))
        {
            await JsonSerializer.SerializeAsync(fileStream, await SingBoxConfigUtils.GenerateClientConfigAsync(s), Global.NewCustomJsonSerializerOptions());
        }

        // 鍚姩 sing-box
        await StartGuardAsync("run -c ..\\data\\last.json");

        return new Socks5Server(IPAddress.Loopback.ToString(), this.Socks5LocalPort(), s.Hostname);
    }
}
