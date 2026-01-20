using System.Web;
using Netch.Models;
using Netch.Utils;

namespace Netch.Servers.Hysteria2;

public static class Hysteria2ShareLink
{
    public static IEnumerable<Server> Parse(string uri)
    {
        if (!uri.StartsWith("hy2://") && !uri.StartsWith("hysteria2://"))
            return Enumerable.Empty<Server>();

        try
        {
            var parsedUri = new Uri(uri);
            var query = HttpUtility.ParseQueryString(parsedUri.Query);

            var server = new Hysteria2Server
            {
                Hostname = parsedUri.Host,
                Port = parsedUri.Port > 0 ? (ushort)parsedUri.Port : (ushort)443,
                Password = Uri.UnescapeDataString(parsedUri.UserInfo),
                ObfuscationType = query["obfs"],
                ObfuscationPassword = query["obfs-password"],
                ServerName = query["sni"],
                // Support both "insecure" and "allowInsecure" parameters
                TLSInsecure = query["insecure"] == "1" || query["allowInsecure"] == "1",
                CertificateFingerprint = query["pinSHA256"],
                PortHoppingRange = query["mport"], // Multi-port parameter
                Remark = !string.IsNullOrEmpty(parsedUri.Fragment)
                    ? Uri.UnescapeDataString(parsedUri.Fragment.TrimStart('#'))
                    : ""
            };

            // Parse ALPN if present, default to h3 for Hysteria2
            var alpn = query["alpn"];
            if (!string.IsNullOrEmpty(alpn))
            {
                server.ALPN = alpn.Split(',');
            }
            else
            {
                // Default ALPN for Hysteria2
                server.ALPN = new[] { "h3" };
            }

            return new[] { server };
        }
        catch (Exception ex)
        {
            Log.Warning($"Failed to parse Hysteria2 URI: {ex.Message}");
            return Enumerable.Empty<Server>();
        }
    }

    public static string Generate(Hysteria2Server server)
    {
        var builder = new UriBuilder
        {
            Scheme = "hy2",
            Host = server.Hostname,
            Port = server.Port == 443 ? -1 : server.Port,
            UserName = Uri.EscapeDataString(server.Password)
        };

        var query = HttpUtility.ParseQueryString(string.Empty);

        if (!string.IsNullOrEmpty(server.ObfuscationType))
            query["obfs"] = server.ObfuscationType;

        if (!string.IsNullOrEmpty(server.ObfuscationPassword))
            query["obfs-password"] = server.ObfuscationPassword;

        if (!string.IsNullOrEmpty(server.ServerName))
            query["sni"] = server.ServerName;

        if (server.TLSInsecure)
            query["insecure"] = "1";

        if (!string.IsNullOrEmpty(server.CertificateFingerprint))
            query["pinSHA256"] = server.CertificateFingerprint;

        if (!string.IsNullOrEmpty(server.PortHoppingRange))
            query["mport"] = server.PortHoppingRange;

        if (server.ALPN != null && server.ALPN.Length > 0)
            query["alpn"] = string.Join(",", server.ALPN);

        builder.Query = query.ToString();

        var uri = builder.Uri.ToString();

        // Add remark as fragment if present
        if (!string.IsNullOrEmpty(server.Remark))
            uri += "#" + Uri.EscapeDataString(server.Remark);

        return uri;
    }
}
