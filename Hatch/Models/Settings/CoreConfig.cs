namespace Hatch.Models;

public enum CoreType
{
    XrayCore,
    SingBox
}

public class CoreConfig
{
    public XrayCoreConfig XrayCore { get; set; } = new();

    public SingBoxConfig SingBox { get; set; } = new();
}

public class XrayCoreConfig
{
    public bool EnableFullCone { get; set; } = true;

    public bool EnableVision { get; set; } = false;

    public bool EnableXHTTP { get; set; } = false;

    public bool EnableReality { get; set; } = false;

    public bool AllowInsecure { get; set; } = false;

    public bool UseMux { get; set; } = false;

    public bool TCPFastOpen { get; set; } = false;

    public KcpConfig KcpConfig { get; set; } = new();
}

public class SingBoxConfig
{
    public bool EnableHysteria2 { get; set; } = true;

    public int Hysteria2UpMbps { get; set; } = 100;

    public int Hysteria2DownMbps { get; set; } = 100;

    public bool UseMultiplex { get; set; } = false;

    public int MultiplexMaxConnections { get; set; } = 8;

    public int MultiplexMinStreams { get; set; } = 4;

    public bool TCPFastOpen { get; set; } = false;
}
