using System;
using System.Drawing;
using System.Windows.Forms;
using System.Collections.Generic;
using Hatch.Forms;

namespace Hatch.Servers;

[Fody.ConfigureAwait(true)]
internal class VLESSForm : ServerForm
{
    public VLESSForm(VLESSServer? server = default)
    {
        server ??= new VLESSServer();
        Server = server;

        var (_, txtUserId) = CreateTextBox("UUID", "UUID", s => true, s => server.UserID = s, server.UserID);
        var genBtn = new Button { Text = "生成", Location = new Point(txtUserId.Right + 5, txtUserId.Top - 1), Size = new Size(50, 25), BackColor = Color.DodgerBlue, ForeColor = Color.White, FlatStyle = FlatStyle.Flat };
        genBtn.FlatAppearance.BorderSize = 0;
        genBtn.Click += (s, e) => txtUserId.Text = Guid.NewGuid().ToString();
        ConfigurationGroupBox.Controls.Add(genBtn);

        CreateComboBox("Flow", "Flow", VLESSGlobal.Flows, s => server.Flow = s, server.Flow ?? "");
        CreateTextBox("EncryptMethod", "Encrypt Method", s => true, s => server.EncryptMethod = !string.IsNullOrWhiteSpace(s) ? s : "none", server.EncryptMethod);
        CreateComboBox("UseMux", "Use Mux", new List<string> { "", "true", "false" }, s => server.UseMux = s switch { "" => null, "true" => true, "false" => false, _ => null }, server.UseMux?.ToString().ToLower() ?? "");
        CreateTextBox("Sni", "ServerName(Sni)", s => true, s => server.ServerName = s, server.ServerName);

        AddDivider("底层传输方式 (transport)");

        CreateComboBox("TransferProtocol", "Transfer Protocol", VLESSGlobal.TransferProtocols, s => server.TransferProtocol = s, server.TransferProtocol);
        CreateComboBox("FakeType", "Fake Type", VLESSGlobal.FakeTypes, s => server.FakeType = s, server.FakeType);
        CreateTextBox("Host", "Host", s => true, s => server.Host = s, server.Host);
        CreateTextBox("Path", "Path", s => true, s => server.Path = s, server.Path);
        CreateComboBox("QUICSecurity", "QUIC Security", VLESSGlobal.QUIC, s => server.QUICSecure = s, server.QUICSecure);
        CreateTextBox("QUICSecret", "QUIC Secret", s => true, s => server.QUICSecret = s, server.QUICSecret);

        AddDivider("传输层安全 (TLS)");

        CreateComboBox("TLSSecure", "TLS Secure", VLESSGlobal.TLSSecure, s => server.TLSSecureType = s, server.TLSSecureType);

        // Reality 相关设置
        CreateTextBox("RealityPublicKey", "Reality Public Key", s => true, s => server.RealityPublicKey = s, server.RealityPublicKey);
        CreateTextBox("RealityShortId", "Reality Short ID", s => true, s => server.RealityShortId = s, server.RealityShortId);
        CreateComboBox("RealityFingerprint", "Reality Fingerprint", VLESSGlobal.Fingerprints, s => server.RealityFingerprint = s, server.RealityFingerprint ?? "chrome");
        CreateTextBox("RealitySpiderX", "Reality SpiderX", s => true, s => server.RealitySpiderX = s, server.RealitySpiderX);

        CreateComboBox("PacketEncoding", "Packet Encoding", VMessGlobal.PacketEncodings, s => server.PacketEncoding = s, server.PacketEncoding);
    }

    protected override string TypeName { get; } = "VLESS";
}