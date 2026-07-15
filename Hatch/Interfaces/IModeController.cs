using Hatch.Models.Modes;
using Hatch.Servers;

namespace Hatch.Interfaces;

public interface IModeController : IController
{
    public ModeFeature Features { get; }

    public Task StartAsync(Socks5Server server, Mode mode);
}