using System.Runtime.InteropServices;

namespace Hatch;

public static class NativeMethods
{
    [DllImport("dnsapi", EntryPoint = "DnsFlushResolverCache")]
    public static extern uint RefreshDNSCache();
}