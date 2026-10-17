using System.Net;
using Hatch.Models;

namespace Hatch.Utils;

public static class SubscriptionUtil
{
    private static readonly object ServerLock = new();

    public static Task UpdateServersAsync(string? proxyServer = default)
    {
        return Task.WhenAll(Global.Settings.Subscription.Select(item => UpdateServerCoreAsync(item, proxyServer)));
    }

    private static async Task UpdateServerCoreAsync(Subscription item, string? proxyServer)
    {
        try
        {
            if (!item.Enable)
                return;

            IWebProxy? proxy = !string.IsNullOrEmpty(proxyServer) ? new WebProxy(proxyServer) : null;
            string? userAgent = !string.IsNullOrEmpty(item.UserAgent) ? item.UserAgent : null;

            using var client = WebUtil.CreateClient(userAgent: userAgent, proxy: proxy);

            List<Server> servers;

            var (code, result) = await WebUtil.DownloadStringAsync(client, item.Link);
            if (code == HttpStatusCode.OK)
                servers = ShareLink.ParseText(result);
            else
                throw new Exception($"{item.Remark} Response Status Code: {code}");

            lock (ServerLock)
            {
                ReplaceServers(Global.Settings.Server, item.Remark, servers);
            }

            Global.MainForm.NotifyTip(i18N.TranslateFormat("Update {1} server(s) from {0}", item.Remark, servers.Count));
        }
        catch (Exception e)
        {
            Global.MainForm.NotifyTip($"{i18N.TranslateFormat("Update servers failed from {0}", item.Remark)}\n{e.Message}", info: false);
            Log.Warning(e, "Update servers failed");
        }
    }

    public static void ReplaceServers(List<Server> existing, string group, List<Server> incoming)
    {
        // Validate the complete update before changing the existing list.
        if (incoming.Count == 0 || incoming.Any(s => string.IsNullOrWhiteSpace(s.Hostname) || s.Port == 0))
            throw new FormatException("Subscription returned no valid servers; existing servers were preserved.");

        foreach (var server in incoming)
            server.Group = group;
        existing.RemoveAll(server => string.Equals(server.Group, group, StringComparison.Ordinal));
        existing.AddRange(incoming);
    }
}
