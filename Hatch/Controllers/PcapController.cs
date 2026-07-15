using System.Text;
using Microsoft.VisualStudio.Threading;
using Hatch.Forms;
using Hatch.Interfaces;
using Hatch.Models;
using Hatch.Models.Modes;
using Hatch.Models.Modes.ShareMode;
using Hatch.Servers;
using Hatch.Utils;

namespace Hatch.Controllers;

public class PcapController : Guard, IModeController, IDisposable
{
    private readonly LogForm _form;
    private ShareMode _mode = null!;
    private Socks5Server _server = null!;
    private bool _disposed;

    public PcapController() : base("pcap2socks.exe", encoding: Encoding.UTF8)
    {
        _form = new LogForm(Global.MainForm);
        _form.CreateControl();
    }

    protected override IEnumerable<string> StartedKeywords { get; } = new[] { "└" };

    public override string Name => "pcap2socks";

    public ModeFeature Features => 0;

    public async Task StartAsync(Socks5Server server, Mode mode)
    {
        if (mode is not ShareMode shareMode)
            throw new InvalidOperationException();

        _server = server;
        _mode = shareMode;

        var outboundNetworkInterface = NetworkInterfaceUtils.GetBest();

        var arguments = new List<object?>
        {
            "--interface", $@"\Device\NPF_{outboundNetworkInterface.Id}",
            "--destination", $"{await _server.AutoResolveHostnameAsync()}:{_server.Port}",
            _mode.Argument, SpecialArgument.Flag
        };

        if (_server.Auth())
            arguments.AddRange(new[]
            {
                "--username", server.Username,
                "--password", server.Password
            });

        await StartGuardAsync(Arguments.Format(arguments));
    }

    public override async Task StopAsync()
    {
        Global.MainForm.Invoke(() => { _form.Close(); });
        await StopGuardAsync();
    }

    public void Dispose()
    {
        Dispose(true);
        GC.SuppressFinalize(this);
    }

    protected virtual void Dispose(bool disposing)
    {
        if (_disposed)
            return;

        if (disposing)
        {
            _form?.Dispose();
        }

        _disposed = true;
    }

    protected override void OnReadNewLine(string line)
    {
        Global.MainForm.BeginInvoke(() =>
        {
            if (!_form.IsDisposed)
                _form.richTextBox1.AppendText(line + "\n");
        });
    }

    protected override void OnStarted()
    {
        Global.MainForm.BeginInvoke(() => _form.Show());
    }

    protected override void OnStartFailed()
    {
        if (new FileInfo(LogPath).Length == 0)
        {
            Task.Run(() =>
                {
                    Thread.Sleep(1000);
                    Utils.Utils.Open("https://github.com/zhxie/pcap2socks#dependencies");
                })
                .ContinueWith(t =>
                {
                    if (t.IsFaulted && t.Exception != null)
                        Log.Error(t.Exception, "Failed to open pcap2socks dependency page");
                });

            throw new MessageException("Pleases install pcap2socks's dependency");
        }

        Utils.Utils.Open(LogPath);
    }
}
