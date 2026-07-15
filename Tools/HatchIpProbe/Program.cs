using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.Net.Http.Headers;
using System.Text.Json;

namespace HatchIpProbe;

internal static class Program
{
    [STAThread]
    private static void Main()
    {
        ApplicationConfiguration.Initialize();
        Application.Run(new MainForm());
    }
}

internal sealed class MainForm : Form
{
    private readonly Button _refreshButton = new();
    private readonly TextBox _outputTextBox = new();
    private readonly Label _statusLabel = new();

    private static readonly Uri[] ProbeUrls =
    {
        new("https://api.ipify.org?format=json"),
        new("https://ifconfig.me/ip"),
        new("https://httpbin.org/ip")
    };

    public MainForm()
    {
        InitializeComponent();
    }

    protected override async void OnShown(EventArgs e)
    {
        base.OnShown(e);
        await RefreshAsync();
    }

    private void InitializeComponent()
    {
        Text = "Hatch IP Probe";
        StartPosition = FormStartPosition.CenterScreen;
        Width = 760;
        Height = 520;
        MinimumSize = new Size(640, 420);

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 3,
            Padding = new Padding(14)
        };
        layout.RowStyles.Add(new RowStyle());
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle());

        var header = new Label
        {
            AutoSize = true,
            Font = new Font(Font.FontFamily, 14, FontStyle.Bold),
            Text = "Hatch IP Probe - 代理出口 IP 测试"
        };

        _outputTextBox.Dock = DockStyle.Fill;
        _outputTextBox.Multiline = true;
        _outputTextBox.ReadOnly = true;
        _outputTextBox.ScrollBars = ScrollBars.Both;
        _outputTextBox.WordWrap = false;
        _outputTextBox.Font = new Font("Consolas", 10);

        var bottomPanel = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            ColumnCount = 2
        };
        bottomPanel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        bottomPanel.ColumnStyles.Add(new ColumnStyle());

        _statusLabel.AutoSize = true;
        _statusLabel.Anchor = AnchorStyles.Left;
        _statusLabel.Text = "准备测试";

        _refreshButton.AutoSize = true;
        _refreshButton.Text = "刷新 IP";
        _refreshButton.Click += async (_, _) => await RefreshAsync();

        bottomPanel.Controls.Add(_statusLabel, 0, 0);
        bottomPanel.Controls.Add(_refreshButton, 1, 0);

        layout.Controls.Add(header, 0, 0);
        layout.Controls.Add(_outputTextBox, 0, 1);
        layout.Controls.Add(bottomPanel, 0, 2);

        Controls.Add(layout);
    }

    private async Task RefreshAsync()
    {
        _refreshButton.Enabled = false;
        _statusLabel.Text = "正在请求公网 IP...";
        _outputTextBox.Clear();
        AppendLine(BuildHeader());

        try
        {
            await AppendDnsResultAsync("api.ipify.org");
            await AppendTcpResultAsync("api.ipify.org", 443);
            AppendLine("");

            using var client = new HttpClient
            {
                Timeout = TimeSpan.FromSeconds(15)
            };
            client.DefaultRequestHeaders.UserAgent.Add(new ProductInfoHeaderValue("HatchIpProbe", "1.0"));

            foreach (var url in ProbeUrls)
            {
                await AppendProbeResultAsync(client, url);
            }

            _statusLabel.Text = "完成。对比开启 Hatch 前后的 IP 是否变化。";
        }
        catch (Exception ex)
        {
            AppendLine("ERROR");
            AppendLine(ex.ToString());
            _statusLabel.Text = "请求失败";
        }
        finally
        {
            _refreshButton.Enabled = true;
        }
    }

    private async Task AppendDnsResultAsync(string host)
    {
        AppendLine($"[{DateTime.Now:HH:mm:ss}] DNS {host}");

        try
        {
            var addresses = await Dns.GetHostAddressesAsync(host);
            AppendLine(addresses.Length == 0 ? "No address returned" : string.Join(", ", addresses.Select(address => address.ToString())));
        }
        catch (Exception ex)
        {
            AppendLine(FormatException(ex));
        }
    }

    private async Task AppendTcpResultAsync(string host, int port)
    {
        AppendLine($"[{DateTime.Now:HH:mm:ss}] TCP {host}:{port}");

        try
        {
            using var tcp = new TcpClient();
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(8));
            await tcp.ConnectAsync(host, port, cts.Token);
            AppendLine($"Connected: {tcp.Client.LocalEndPoint} -> {tcp.Client.RemoteEndPoint}");
        }
        catch (Exception ex)
        {
            AppendLine(FormatException(ex));
        }
    }

    private async Task AppendProbeResultAsync(HttpClient client, Uri url)
    {
        AppendLine($"[{DateTime.Now:HH:mm:ss}] GET {url}");

        try
        {
            var text = await client.GetStringAsync(url);
            AppendLine(ParseIp(text));
        }
        catch (Exception ex)
        {
            AppendLine(FormatException(ex));
        }

        AppendLine("");
    }

    private static string ParseIp(string text)
    {
        var trimmed = text.Trim();
        if (!trimmed.StartsWith("{"))
            return trimmed;

        using var document = JsonDocument.Parse(trimmed);
        if (document.RootElement.TryGetProperty("ip", out var ip))
            return ip.GetString() ?? trimmed;
        if (document.RootElement.TryGetProperty("origin", out var origin))
            return origin.GetString() ?? trimmed;

        return trimmed;
    }

    private static string BuildHeader()
    {
        return $"""
               Process: {Process.GetCurrentProcess().ProcessName}
               Path:    {Environment.ProcessPath}
               Time:    {DateTimeOffset.Now}

               """;
    }

    private static string FormatException(Exception ex)
    {
        var parts = new List<string>();
        for (var current = ex; current != null; current = current.InnerException)
            parts.Add($"{current.GetType().Name}: {current.Message}");

        return "FAILED: " + string.Join(" -> ", parts);
    }

    private void AppendLine(string value)
    {
        _outputTextBox.AppendText(value + Environment.NewLine);
    }
}
