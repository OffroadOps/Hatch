using System.Text.Json;
using Netch.Controllers;
using Netch.Interfaces;
using Netch.Models;

namespace Netch.Servers.Hysteria2;

public class Hysteria2Controller : Guard, IServerController
{
    private string? _configPath;

    public Hysteria2Controller() : base("sing-box.exe")
    {
    }

    protected override IEnumerable<string> StartedKeywords => new[] { "sing-box started" };

    protected override IEnumerable<string> FailedKeywords => new[] { "FATAL", "failed to", "error" };

    public override string Name => "sing-box";

    public ushort? Socks5LocalPort { get; set; }

    public string? LocalAddress { get; set; }

    public async Task<Socks5Server> StartAsync(Server s)
    {
        var server = (Hysteria2Server)s;

        // Generate sing-box configuration
        var config = GenerateConfig(server);

        // Write config to temporary file
        _configPath = Path.Combine(Global.NetchDir, "data", "sing-box-hysteria2.json");
        var configJson = JsonSerializer.Serialize(config, Global.NewCustomJsonSerializerOptions());
        
        // Log configuration for debugging
        Log.Information($"Hysteria2 Config: {configJson}");
        
        await File.WriteAllTextAsync(_configPath, configJson);

        // Start sing-box process
        await StartGuardAsync($"run -c \"{_configPath}\"");

        // Return SOCKS5 proxy details
        return new Socks5Server(
            this.LocalAddress(),
            this.Socks5LocalPort(),
            server.Hostname);
    }

    public override async Task StopAsync()
    {
        await base.StopAsync();

        // Clean up temporary configuration file
        if (_configPath != null && File.Exists(_configPath))
        {
            try
            {
                File.Delete(_configPath);
            }
            catch
            {
                // Ignore cleanup errors
            }
        }
    }

    private object GenerateConfig(Hysteria2Server server)
    {
        var config = new
        {
            log = new { level = "info" },
            inbounds = new[]
            {
                new
                {
                    type = "socks",
                    tag = "socks-in",
                    listen = this.LocalAddress(),
                    listen_port = this.Socks5LocalPort(),
                    sniff = true,
                    sniff_override_destination = false
                }
            },
            outbounds = new[]
            {
                CreateHysteria2Outbound(server)
            }
        };

        return config;
    }

    private object CreateHysteria2Outbound(Hysteria2Server server)
    {
        var outbound = new Dictionary<string, object>
        {
            ["type"] = "hysteria2",
            ["tag"] = "proxy",
            ["server"] = server.Hostname,
            ["server_port"] = server.Port,
            ["password"] = server.Password
        };

        // Bandwidth (Brutal CC)
        if (server.UploadBandwidth.HasValue)
            outbound["up_mbps"] = server.UploadBandwidth.Value;
        if (server.DownloadBandwidth.HasValue)
            outbound["down_mbps"] = server.DownloadBandwidth.Value;

        // Obfuscation
        if (!string.IsNullOrEmpty(server.ObfuscationType))
        {
            outbound["obfs"] = new
            {
                type = server.ObfuscationType,
                password = server.ObfuscationPassword
            };
        }

        // Port Hopping - 注意：当前版本的 sing-box 可能不支持端口跳跃
        // 暂时忽略端口跳跃配置，使用主端口
        if (!string.IsNullOrEmpty(server.PortHoppingRange))
        {
            Log.Warning($"Port hopping is configured ({server.PortHoppingRange}) but may not be supported by sing-box. Using main port only.");
            // TODO: 研究 sing-box 的端口跳跃支持
        }

        // TLS
        var tls = new Dictionary<string, object>
        {
            ["enabled"] = true
        };

        if (!string.IsNullOrEmpty(server.ServerName))
            tls["server_name"] = server.ServerName;
        if (server.TLSInsecure)
            tls["insecure"] = true;
        if (server.ALPN != null && server.ALPN.Length > 0)
            tls["alpn"] = server.ALPN;
        
        // Certificate fingerprint (SHA256) - requires sing-box 1.13.0+
        if (!string.IsNullOrEmpty(server.CertificateFingerprint))
        {
            tls["certificate_public_key_sha256"] = new[] { server.CertificateFingerprint };
        }

        outbound["tls"] = tls;

        return outbound;
    }
}
