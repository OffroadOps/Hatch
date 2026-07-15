using System.Collections.Concurrent;
using System.Net;
using System.Net.Sockets;

namespace Hatch.Utils;

public static class DnsUtils
{
    /// <summary>
    ///     DNS 缓存 TTL（5 分钟）
    /// </summary>
    private static readonly TimeSpan CacheTtl = TimeSpan.FromMinutes(5);

    private record CacheEntry(IPAddress Address, DateTime ExpiresAt);

    private static readonly ConcurrentDictionary<string, CacheEntry> Cache = new();
    private static readonly ConcurrentDictionary<string, CacheEntry> Cache6 = new();

    public static async Task<IPAddress?> LookupAsync(string hostname, AddressFamily inet = AddressFamily.Unspecified, int timeout = 3000)
    {
        try
        {
            var now = DateTime.UtcNow;

            var cacheResult = inet switch
            {
                AddressFamily.Unspecified => TryGetValid(Cache, hostname, now) ?? TryGetValid(Cache6, hostname, now),
                AddressFamily.InterNetwork => TryGetValid(Cache, hostname, now),
                AddressFamily.InterNetworkV6 => TryGetValid(Cache6, hostname, now),
                _ => throw new ArgumentOutOfRangeException()
            };

            if (cacheResult != null)
                return cacheResult;

            return await LookupNoCacheAsync(hostname, inet, timeout);
        }
        catch (Exception e)
        {
            Log.Verbose(e, "Lookup hostname {Hostname} failed", hostname);
            return null;
        }
    }

    private static IPAddress? TryGetValid(ConcurrentDictionary<string, CacheEntry> cache, string key, DateTime now)
    {
        if (cache.TryGetValue(key, out var entry))
        {
            if (entry.ExpiresAt > now)
                return entry.Address;

            // 过期，移除
            cache.TryRemove(key, out _);
        }
        return null;
    }

    private static async Task<IPAddress?> LookupNoCacheAsync(string hostname, AddressFamily inet = AddressFamily.Unspecified, int timeout = 3000)
    {
        using var task = Dns.GetHostAddressesAsync(hostname);
        using var resTask = await Task.WhenAny(task, Task.Delay(timeout));

        if (resTask == task)
        {
            var addresses = await task;

            var result = addresses.FirstOrDefault(i => inet == AddressFamily.Unspecified || inet == i.AddressFamily);
            if (result == null)
                return null;

            var entry = new CacheEntry(result, DateTime.UtcNow + CacheTtl);

            switch (result.AddressFamily)
            {
                case AddressFamily.InterNetwork:
                    Cache[hostname] = entry;
                    break;
                case AddressFamily.InterNetworkV6:
                    Cache6[hostname] = entry;
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }

            return result;
        }

        return null;
    }

    public static void ClearCache()
    {
        Cache.Clear();
        Cache6.Clear();
    }

    public static string AppendPort(string host, ushort port = 53)
    {
        if (!host.Contains(':'))
            return host + $":{port}";

        return host;
    }
}
