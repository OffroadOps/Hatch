using System.Diagnostics;
using Microsoft.VisualStudio.Threading;
using Hatch.Interfaces;
using Hatch.Models;
using Hatch.Models.Modes;
using Hatch.Servers;
using Hatch.Servers.SingBox;
using Hatch.Services;
using Hatch.Utils;

namespace Hatch.Controllers;

public static class MainController
{
    public static Socks5Server? Socks5Server { get; private set; }

    public static Server? Server { get; private set; }

    public static Mode? Mode { get; private set; }

    public static IServerController? ServerController { get; private set; }

    public static IModeController? ModeController { get; private set; }

    private static readonly AsyncSemaphore Lock = new(1);

    public static async Task StartAsync(Server server, Mode mode)
    {
        using var releaser = await Lock.EnterAsync();

        Log.Information("Start MainController: {Server} {Mode}", $"{server.Type}", $"[{(int)mode.Type}]{mode.i18NRemark}");

        if (await DnsUtils.LookupAsync(server.Hostname) == null)
            throw new MessageException(i18N.Translate("Lookup Server hostname failed"));

        // TODO Disable NAT Type Test setting
        // cache STUN Server ip to prevent "Wrong STUN Server"
        DnsUtils.LookupAsync(Global.Settings.STUN_Server).ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Warning(t.Exception, "Failed to cache STUN server DNS");
        });

        Server = server;
        Mode = mode;

        await Task.WhenAll(Task.Run(NativeMethods.RefreshDNSCache), Task.Run(Firewall.AddHatchFwRules));

        try
        {
            ModeController = ModeService.GetModeControllerByType(mode.Type, out var modePort, out var portName);

            if (modePort != null)
                TryReleaseTcpPort((ushort)modePort, portName);

            if (Server is Socks5Server socks5 && (!socks5.Auth() || ModeController.Features.HasFlag(ModeFeature.SupportSocks5Auth)))
            {
                Socks5Server = socks5;
            }
            else
            {
                // Start Server Controller to get a local socks5 server
                Log.Debug("Server Information: {Data}", $"{server.Type} {server.MaskedData()}");

                // Automatically select the appropriate core based on server protocol
                var requiredCore = server.GetRequiredCore();
                var coreFullName = CoreSelector.GetCoreFullName(requiredCore);

                Log.Information("Auto-selected core: {Core} for {Protocol}", coreFullName, server.Type);

                // Check if the required core is available
                if (!CoreSelector.IsCoreAvailable(requiredCore))
                {
                    var coreExecutable = CoreSelector.GetCoreExecutableName(requiredCore);
                    throw new FileNotFoundException(
                        $"{i18N.Translate("Missing required core")}\n\n" +
                        $"{i18N.TranslateFormat("Need {0} to run {1} protocol", coreFullName, server.Type)}\n" +
                        $"{i18N.TranslateFormat("Missing file: bin\\{0}", coreExecutable)}\n\n" +
                        $"{i18N.Translate("Please download the complete version from the project homepage.")}"
                    );
                }

                // Create the appropriate controller based on the selected core
                ServerController = requiredCore switch
                {
                    CoreType.XrayCore => new XrayController(),
                    CoreType.SingBox => new SingBoxController(),
                    _ => throw new NotSupportedException($"Core type {requiredCore} is not supported")
                };

                ServerController.Socks5LocalPort = Global.Settings.Socks5LocalPort;
                ServerController.LocalAddress = Global.Settings.LocalAddress;

                Global.MainForm.StatusText(i18N.TranslateFormat("Starting {0}", ServerController.Name));

                TryReleaseTcpPort(ServerController.Socks5LocalPort(), "Socks5");
                Socks5Server = await ServerController.StartAsync(server);

                StatusPortInfoText.Socks5Port = Socks5Server.Port;
                StatusPortInfoText.UpdateShareLan();
            }

            // Start Mode Controller
            Global.MainForm.StatusText(i18N.TranslateFormat("Starting {0}", ModeController.Name));

            await ModeController.StartAsync(Socks5Server, mode);
        }
        catch (Exception e)
        {
            releaser.Dispose();
            await StopAsync();

            switch (e)
            {
                case DllNotFoundException:
                case FileNotFoundException:
                    throw new Exception(e.Message + "\n\n" + i18N.Translate("Missing File or runtime components"));
                case MessageException:
                    throw;
                default:
                    Log.Error(e, "Unhandled Exception When Start MainController");
                    Utils.Utils.Open(Constants.LogFile);
                    throw new MessageException($"{i18N.Translate("Unhandled Exception")}\n{e.Message}");
            }
        }
    }

    public static async Task StopAsync()
    {
        if (Lock.CurrentCount == 0)
        {
            (await Lock.EnterAsync()).Dispose();
            if (ServerController == null && ModeController == null)
                // stopped
                return;

            // else begin stop
        }

        using var _ = await Lock.EnterAsync();

        if (ServerController == null && ModeController == null)
            return;

        Log.Information("Stop Main Controller");
        StatusPortInfoText.Reset();

        var tasks = new[]
        {
            ServerController?.StopAsync() ?? Task.CompletedTask,
            ModeController?.StopAsync() ?? Task.CompletedTask
        };

        try
        {
            await Task.WhenAll(tasks);
        }
        catch (Exception e)
        {
            Log.Error(e, "MainController Stop Error");
        }

        ServerController = null;
        ModeController = null;
    }

    public static void PortCheck(ushort port, string portName, PortType portType = PortType.Both)
    {
        try
        {
            PortHelper.CheckPort(port, portType);
        }
        catch (PortInUseException)
        {
            throw new MessageException(i18N.TranslateFormat("The {0} port is in use.", $"{portName} ({port})"));
        }
        catch (PortReservedException)
        {
            throw new MessageException(i18N.TranslateFormat("The {0} port is reserved by system.", $"{portName} ({port})"));
        }
    }

    public static void TryReleaseTcpPort(ushort port, string portName)
    {
        foreach (var p in PortHelper.GetProcessByUsedTcpPort(port))
        {
            var fileName = p.MainModule?.FileName;
            if (fileName == null)
                continue;

            if (fileName.StartsWith(Global.HatchDir))
            {
                p.Kill();
                p.WaitForExit();
            }
            else
            {
                throw new MessageException(i18N.TranslateFormat("The {0} port is used by {1}.", $"{portName} ({port})", $"({p.Id}){fileName}"));
            }
        }

        PortCheck(port, portName, PortType.TCP);
    }

    public static Task<NatTypeTestResult> DiscoveryNatTypeAsync(CancellationToken ctx = default)
    {
        Debug.Assert(Socks5Server != null, nameof(Socks5Server) + " != null");
        return Socks5ServerTestUtils.DiscoveryNatTypeAsync(Socks5Server, ctx);
    }

    public static Task<int?> HttpConnectAsync(CancellationToken ctx = default)
    {
        Debug.Assert(Socks5Server != null, nameof(Socks5Server) + " != null");
        try
        {
            return Socks5ServerTestUtils.HttpConnectAsync(Socks5Server, ctx);
        }
        catch (OperationCanceledException)
        {
            // 用户取消操作，正常情况
            return Task.FromResult<int?>(null);
        }
        catch (Exception e)
        {
            Log.Warning(e, "HTTP connect test failed");
            return Task.FromResult<int?>(null);
        }
    }
}
