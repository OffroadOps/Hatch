using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;
using Hatch;
using Hatch.Models;
using Hatch.Servers;
using Hatch.Servers.Hysteria2;
using Hatch.Servers.SingBox;
using Hatch.Utils;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Tests;

[TestClass]
public class ProxyRegressionTests
{
    [TestMethod]
    public void HatchLink_RoundTripsNumericPortAndCredentials()
    {
        var server = new Socks5Server { Hostname = "127.0.0.1", Port = 1080, Username = "test", Password = "test-secret", Remark = "test" };
        var link = ShareLink.GetHatchLink(server);
        StringAssert.StartsWith(link, "NPGA://");
        var result = ShareLink.ParseText(link);
        Assert.AreEqual(1, result.Count);
        var parsed = (Socks5Server)result[0];
        Assert.AreEqual(server.Hostname, parsed.Hostname);
        Assert.AreEqual(server.Port, parsed.Port);
        Assert.AreEqual(server.Username, parsed.Username);
        Assert.AreEqual(server.Password, parsed.Password);
        Assert.AreEqual(1, ShareLink.ParseText(link.Replace("NPGA://", "Hatch://")).Count);
    }

    [TestMethod]
    public void HatchLink_AcceptsLegacyStringPortAndRejectsZeroPort()
    {
        const string json = "{\"Type\":\"SOCKS\",\"Hostname\":\"127.0.0.1\",\"Port\":\"1080\"}";
        Assert.AreEqual(1, ShareLink.ParseText("Hatch://" + ShareLink.URLSafeBase64Encode(json)).Count);
        Assert.AreEqual(0, ShareLink.ParseText("Hatch://" + ShareLink.URLSafeBase64Encode(json.Replace("1080", "0"))).Count);
    }

    [TestMethod]
    public void InvalidSubscription_PreservesPreviousServers()
    {
        var original = new Socks5Server { Hostname = "127.0.0.1", Port = 1080, Group = "subscription" };
        var existing = new List<Server> { original };
        var incoming = ShareLink.ParseText("<html>Service temporarily unavailable</html>");
        Assert.ThrowsException<FormatException>(() => SubscriptionUtil.ReplaceServers(existing, "subscription", incoming));
        Assert.AreEqual(1, existing.Count);
        Assert.AreSame(original, existing[0]);
        Assert.ThrowsException<FormatException>(() => SubscriptionUtil.ReplaceServers(existing, "subscription", [new Socks5Server()]));
        Assert.AreSame(original, existing[0]);
    }

    [TestMethod]
    public void ValidSubscription_ReplacesOnlyTargetGroup()
    {
        var other = new Socks5Server { Group = "manual", Hostname = "127.0.0.1", Port = 1081 };
        var existing = new List<Server> { other, new Socks5Server { Group = "subscription" } };
        var incoming = new Socks5Server { Hostname = "127.0.0.1", Port = 1080 };
        SubscriptionUtil.ReplaceServers(existing, "subscription", [incoming]);
        Assert.AreEqual(2, existing.Count);
        Assert.AreSame(other, existing[0]);
        Assert.AreEqual("subscription", incoming.Group);
    }

    [TestMethod]
    public async Task SingBoxConfig_UsesRouteActionAndCanBeExportedForCoreCheck()
    {
        var config = await SingBoxConfigUtils.GenerateClientConfigAsync(new Hysteria2Server
        {
            Hostname = "127.0.0.1", Port = 443, Password = "test-secret", ServerName = "example.com"
        });
        var json = JsonSerializer.Serialize(config, Global.NewCustomJsonSerializerOptions());
        using var document = JsonDocument.Parse(json);
        Assert.IsFalse(document.RootElement.GetProperty("inbounds")[0].TryGetProperty("sniff", out _));
        Assert.AreEqual("sniff", document.RootElement.GetProperty("route").GetProperty("rules")[0].GetProperty("action").GetString());
        var output = Environment.GetEnvironmentVariable("HATCH_CONFIG_CHECK_OUTPUT");
        if (!string.IsNullOrEmpty(output)) await File.WriteAllTextAsync(output, json);
    }
}
