using System.Text.RegularExpressions;
using System.Web;
using Netch.Models;
using Netch.Utils;

namespace Netch.Servers;

public static class V2rayUtils
{
    public static IEnumerable<Server> ParseVUri(string text)
    {
        var scheme = ShareLink.GetUriScheme(text).ToLower();
        var server = scheme switch { "vmess" => new VMessServer(), "vless" => new VLESSServer(), _ => throw new ArgumentOutOfRangeException() };
        if (text.Contains("#"))
        {
            server.Remark = Uri.UnescapeDataString(text.Split('#')[1]);
            text = text.Split('#')[0];
        }

        if (text.Contains("?"))
        {
            var parameter = HttpUtility.ParseQueryString(text.Split('?')[1]);
            text = text.Substring(0, text.IndexOf("?", StringComparison.Ordinal));
            server.TransferProtocol = parameter.Get("type") ?? "tcp";
            server.PacketEncoding = parameter.Get("packetEncoding") ?? "xudp";
            server.EncryptMethod = parameter.Get("encryption") ?? scheme switch { "vless" => "none", _ => "auto" };
            switch (server.TransferProtocol)
            {
                case "tcp":
                    break;
                case "kcp":
                    server.FakeType = parameter.Get("headerType") ?? "none";
                    server.Path = Uri.UnescapeDataString(parameter.Get("seed") ?? "");
                    break;
                case "ws":
                    server.Path = Uri.UnescapeDataString(parameter.Get("path") ?? "/");
                    server.Host = Uri.UnescapeDataString(parameter.Get("host") ?? "");
                    break;
                case "h2":
                    server.Path = Uri.UnescapeDataString(parameter.Get("path") ?? "/");
                    server.Host = Uri.UnescapeDataString(parameter.Get("host") ?? "");
                    break;
                case "quic":
                    server.QUICSecure = parameter.Get("quicSecurity") ?? "none";
                    server.QUICSecret = parameter.Get("key") ?? "";
                    server.FakeType = parameter.Get("headerType") ?? "none";
                    break;
                case "grpc":
                    server.FakeType = parameter.Get("mode") ?? "gun";
                    server.Path = parameter.Get("serviceName") ?? "";
                    break;
            }

            server.TLSSecureType = parameter.Get("security") ?? "none";
            if (server.TLSSecureType != "none")
            {
                server.ServerName = parameter.Get("sni") ?? "";
            }

            // Reality 参数解析
            if (server.TLSSecureType == "reality" && server is VLESSServer vlessServer)
            {
                vlessServer.RealityPublicKey = parameter.Get("pbk") ?? "";
                vlessServer.RealityShortId = parameter.Get("sid") ?? "";
                vlessServer.RealityFingerprint = parameter.Get("fp") ?? "chrome";
                vlessServer.RealitySpiderX = Uri.UnescapeDataString(parameter.Get("spx") ?? "");
                vlessServer.Flow = parameter.Get("flow") ?? "";
                // Reality 使用 sni 作为 serverName
                vlessServer.ServerName = parameter.Get("sni") ?? "";
            }
            else if (server is VLESSServer vless2)
            {
                // 非 Reality 的 flow 参数
                vless2.Flow = parameter.Get("flow") ?? "";
            }
        }

        var finder = new Regex(@$"^{scheme}://(?<guid>.+?)@(?<server>.+):(?<port>\d+)");
        var match = finder.Match(text.Split('?')[0]);
        if (!match.Success)
            throw new FormatException();

        server.UserID = match.Groups["guid"].Value;
        server.Hostname = match.Groups["server"].Value;
        server.Port = ushort.Parse(match.Groups["port"].Value);

        return new[] { server };
    }

    public static string GetVShareLink(Server s, string scheme = "vmess")
    {
        // https://github.com/XTLS/Xray-core/issues/91
        var server = (VMessServer)s;
        var parameter = new Dictionary<string, string>();
        // protocol-specific fields
        parameter.Add("type", server.TransferProtocol);
        parameter.Add("encryption", server.EncryptMethod);
        parameter.Add("packetEncoding", server.PacketEncoding);

        // transport-specific fields
        switch (server.TransferProtocol)
        {
            case "tcp":
                break;
            case "kcp":
                if (server.FakeType != "none")
                    parameter.Add("headerType", server.FakeType);

                if (!server.Path.IsNullOrWhiteSpace())
                    parameter.Add("seed", Uri.EscapeDataString(server.Path!));

                break;
            case "ws":
                parameter.Add("path", Uri.EscapeDataString(server.Path.ValueOrDefault() ?? "/"));
                if (!server.Host.IsNullOrWhiteSpace())
                    parameter.Add("host", Uri.EscapeDataString(server.Host!));

                break;
            case "h2":
                parameter.Add("path", Uri.EscapeDataString(server.Path.ValueOrDefault() ?? "/"));
                if (!server.Host.IsNullOrWhiteSpace())
                    parameter.Add("host", Uri.EscapeDataString(server.Host!));

                break;
            case "quic":
                if (server.QUICSecure is not (null or "none"))
                {
                    parameter.Add("quicSecurity", server.QUICSecure);
                    parameter.Add("key", server.QUICSecret!);
                }

                if (server.FakeType != "none")
                    parameter.Add("headerType", server.FakeType);

                break;
            case "grpc":
                if (!string.IsNullOrEmpty(server.Path))
                    parameter.Add("serviceName", server.Path);

                if (server.FakeType is "gun" or "multi")
                    parameter.Add("mode", server.FakeType);

                break;
        }

        if (server.TLSSecureType != "none")
        {
            parameter.Add("security", server.TLSSecureType);

            if (!server.Host.IsNullOrWhiteSpace())
                parameter.Add("sni", server.Host!);

            if (server.TLSSecureType == "xtls")
            {
                parameter.Add("flow", "xtls-rprx-direct");
            }

            // Reality 参数
            if (server.TLSSecureType == "reality" && server is VLESSServer vless)
            {
                if (!string.IsNullOrEmpty(vless.RealityPublicKey))
                    parameter.Add("pbk", vless.RealityPublicKey);
                if (!string.IsNullOrEmpty(vless.RealityShortId))
                    parameter.Add("sid", vless.RealityShortId);
                if (!string.IsNullOrEmpty(vless.RealityFingerprint))
                    parameter.Add("fp", vless.RealityFingerprint);
                if (!string.IsNullOrEmpty(vless.RealitySpiderX))
                    parameter.Add("spx", Uri.EscapeDataString(vless.RealitySpiderX));
                if (!string.IsNullOrEmpty(vless.Flow))
                    parameter.Add("flow", vless.Flow);
            }
        }

        return
            $"{scheme}://{server.UserID}@{server.Hostname}:{server.Port}?{string.Join("&", parameter.Select(p => $"{p.Key}={p.Value}"))}{(!server.Remark.IsNullOrWhiteSpace() ? $"#{Uri.EscapeDataString(server.Remark)}" : "")}";
    }
}