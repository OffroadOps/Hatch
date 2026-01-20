using Netch.Forms;

namespace Netch.Servers.Hysteria2;

[Fody.ConfigureAwait(true)]
internal class Hysteria2Form : ServerForm
{
    public Hysteria2Form(Hysteria2Server? server = default)
    {
        server ??= new Hysteria2Server();
        Server = server;

        CreateTextBox("Password",
            "Password",
            s => !string.IsNullOrWhiteSpace(s),
            s => server.Password = s,
            server.Password);

        CreateTextBox("ServerName",
            "Server Name (SNI)",
            s => true,
            s => server.ServerName = string.IsNullOrWhiteSpace(s) ? null : s,
            server.ServerName ?? "");

        CreateTextBox("UploadBandwidth",
            "Upload (Mbps)",
            s => string.IsNullOrEmpty(s) || (int.TryParse(s, out var v) && v > 0),
            s => server.UploadBandwidth = string.IsNullOrEmpty(s) ? null : int.Parse(s),
            server.UploadBandwidth?.ToString() ?? "");

        CreateTextBox("DownloadBandwidth",
            "Download (Mbps)",
            s => string.IsNullOrEmpty(s) || (int.TryParse(s, out var v) && v > 0),
            s => server.DownloadBandwidth = string.IsNullOrEmpty(s) ? null : int.Parse(s),
            server.DownloadBandwidth?.ToString() ?? "");

        CreateComboBox("ObfuscationType",
            "Obfuscation",
            new List<string> { "", "salamander" },
            s => server.ObfuscationType = string.IsNullOrEmpty(s) ? null : s,
            server.ObfuscationType ?? "");

        CreateTextBox("ObfuscationPassword",
            "Obfuscation Password",
            s => true,
            s => server.ObfuscationPassword = string.IsNullOrWhiteSpace(s) ? null : s,
            server.ObfuscationPassword ?? "");

        CreateCheckBox("TLSInsecure",
            "Skip TLS Verification",
            b => server.TLSInsecure = b,
            server.TLSInsecure);

        CreateTextBox("CertificateFingerprint",
            "Certificate Fingerprint (SHA256)",
            s => true,
            s => server.CertificateFingerprint = string.IsNullOrWhiteSpace(s) ? null : s,
            server.CertificateFingerprint ?? "");

        CreateTextBox("ALPN",
            "ALPN (comma separated, e.g., h3,h2)",
            s => true,
            s => server.ALPN = string.IsNullOrWhiteSpace(s) ? null : s.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries),
            server.ALPN != null ? string.Join(",", server.ALPN) : "");

        CreateTextBox("PortHoppingRange",
            "Port Hopping Range",
            s => true,
            s => server.PortHoppingRange = string.IsNullOrWhiteSpace(s) ? null : s,
            server.PortHoppingRange ?? "");

        CreateTextBox("HopInterval",
            "Hop Interval",
            s => true,
            s => server.HopInterval = string.IsNullOrWhiteSpace(s) ? null : s,
            server.HopInterval ?? "");
    }

    protected override string TypeName { get; } = "Hysteria2";
}
