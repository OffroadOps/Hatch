using Netch.Interfaces;
using Netch.Models;

namespace Netch.Servers.Hysteria2;

public class Hysteria2Util : IServerUtil
{
    public ushort Priority { get; } = 3;

    public string TypeName { get; } = "Hysteria2";

    public string FullName { get; } = "Hysteria2";

    public string ShortName { get; } = "HY2";

    public string[] UriScheme { get; } = { "hysteria2", "hy2" };

    public Type ServerType { get; } = typeof(Hysteria2Server);

    public void Edit(Server s)
    {
        new Hysteria2Form((Hysteria2Server)s).ShowDialog();
    }

    public void Create()
    {
        new Hysteria2Form().ShowDialog();
    }

    public string GetShareLink(Server s)
    {
        return Hysteria2ShareLink.Generate((Hysteria2Server)s);
    }

    public IServerController GetController()
    {
        return new Hysteria2Controller();
    }

    public IEnumerable<Server> ParseUri(string text)
    {
        return Hysteria2ShareLink.Parse(text);
    }

    public bool CheckServer(Server s)
    {
        var server = (Hysteria2Server)s;
        return !string.IsNullOrWhiteSpace(server.Password);
    }
}
