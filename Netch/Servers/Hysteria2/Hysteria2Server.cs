using Netch.Models;

namespace Netch.Servers.Hysteria2;

public class Hysteria2Server : Server
{
    public override string Type { get; } = "Hysteria2";

    /// <summary>
    ///     Authentication Password
    /// </summary>
    public string Password { get; set; } = string.Empty;

    /// <summary>
    ///     Upload Bandwidth in Mbps (null = use BBR instead of Brutal)
    /// </summary>
    public int? UploadBandwidth { get; set; }

    /// <summary>
    ///     Download Bandwidth in Mbps (null = use BBR instead of Brutal)
    /// </summary>
    public int? DownloadBandwidth { get; set; }

    /// <summary>
    ///     Obfuscation Type (e.g., "salamander")
    /// </summary>
    public string? ObfuscationType { get; set; }

    /// <summary>
    ///     Obfuscation Password
    /// </summary>
    public string? ObfuscationPassword { get; set; }

    /// <summary>
    ///     TLS Server Name (SNI)
    /// </summary>
    public string? ServerName { get; set; }

    /// <summary>
    ///     Skip TLS Verification
    /// </summary>
    public bool TLSInsecure { get; set; } = false;

    /// <summary>
    ///     TLS ALPN Protocols
    /// </summary>
    public string[]? ALPN { get; set; }

    /// <summary>
    ///     Certificate Fingerprint (SHA256)
    /// </summary>
    public string? CertificateFingerprint { get; set; }

    /// <summary>
    ///     Port Hopping Range (e.g., "2000-3000")
    /// </summary>
    public string? PortHoppingRange { get; set; }

    /// <summary>
    ///     Hop Interval (e.g., "30s")
    /// </summary>
    public string? HopInterval { get; set; }

    public override string MaskedData()
    {
        var parts = new List<string>();

        if (UploadBandwidth.HasValue && DownloadBandwidth.HasValue)
            parts.Add($"Brutal ({UploadBandwidth}/{DownloadBandwidth} Mbps)");
        else
            parts.Add("BBR");

        if (!string.IsNullOrEmpty(ObfuscationType))
            parts.Add($"Obfs: {ObfuscationType}");

        if (!string.IsNullOrEmpty(ServerName))
            parts.Add($"SNI: {ServerName}");

        return parts.Count > 0 ? string.Join(" + ", parts) : "Default";
    }
}
