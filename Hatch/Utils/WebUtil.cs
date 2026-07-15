using System.Net;
using System.Net.Http.Headers;
using System.Text;

namespace Hatch.Utils;

public static class WebUtil
{
    public const string DefaultUserAgent =
        @"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/94.0.4606.61 Safari/537.36 Edg/94.0.992.31";

    private static readonly HttpClient DefaultClient;

    static WebUtil()
    {
        var handler = new SocketsHttpHandler
        {
            AutomaticDecompression = DecompressionMethods.All,
            PooledConnectionLifetime = TimeSpan.FromMinutes(10)
        };

        DefaultClient = new HttpClient(handler);
        DefaultClient.DefaultRequestHeaders.Accept.ParseAdd("*/*");
        DefaultClient.DefaultRequestHeaders.AcceptCharset.Add(new StringWithQualityHeaderValue("utf-8"));
        DefaultClient.DefaultRequestHeaders.UserAgent.ParseAdd(DefaultUserAgent);
    }

    private static int DefaultGetTimeout => Global.Settings.RequestTimeout;

    /// <summary>
    /// 创建带自定义选项的 HttpClient（用于需要设置代理或自定义 UserAgent 的场景）
    /// </summary>
    public static HttpClient CreateClient(int? timeout = null, string? userAgent = null, IWebProxy? proxy = null)
    {
        var handler = new SocketsHttpHandler
        {
            AutomaticDecompression = DecompressionMethods.All,
            PooledConnectionLifetime = TimeSpan.FromMinutes(10)
        };

        if (proxy != null)
            handler.Proxy = proxy;

        var client = new HttpClient(handler)
        {
            Timeout = TimeSpan.FromMilliseconds(timeout ?? DefaultGetTimeout)
        };

        client.DefaultRequestHeaders.Accept.ParseAdd("*/*");
        client.DefaultRequestHeaders.AcceptCharset.Add(new StringWithQualityHeaderValue("utf-8"));
        client.DefaultRequestHeaders.UserAgent.Clear();
        client.DefaultRequestHeaders.UserAgent.ParseAdd(
            string.IsNullOrWhiteSpace(userAgent) ? DefaultUserAgent : userAgent);

        return client;
    }

    /// <summary>
    /// 下载字节数组
    /// </summary>
    public static async Task<byte[]> DownloadBytesAsync(string url, int? timeout = null)
    {
        using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(timeout ?? DefaultGetTimeout));
        return await DefaultClient.GetByteArrayAsync(url, cts.Token);
    }

    /// <summary>
    /// 下载字符串（使用默认 HttpClient）
    /// </summary>
    public static async Task<(HttpStatusCode, string)> DownloadStringAsync(string url, Encoding? encoding = null, int? timeout = null)
    {
        using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(timeout ?? DefaultGetTimeout));
        using var response = await DefaultClient.GetAsync(url, cts.Token);
        var content = encoding != null
            ? encoding.GetString(await response.Content.ReadAsByteArrayAsync(cts.Token))
            : await response.Content.ReadAsStringAsync(cts.Token);
        return (response.StatusCode, content);
    }

    /// <summary>
    /// 使用自定义 HttpClient 下载字符串（支持代理、自定义 UserAgent）
    /// </summary>
    public static async Task<(HttpStatusCode, string)> DownloadStringAsync(HttpClient client, string url, Encoding? encoding = null)
    {
        using var response = await client.GetAsync(url);
        var content = encoding != null
            ? encoding.GetString(await response.Content.ReadAsByteArrayAsync())
            : await response.Content.ReadAsStringAsync();
        return (response.StatusCode, content);
    }

    /// <summary>
    /// 下载文件到指定路径
    /// </summary>
    public static Task DownloadFileAsync(string url, string fileFullPath, IProgress<int>? progress = null)
    {
        return DownloadFileAsync(DefaultClient, url, fileFullPath, progress);
    }

    /// <summary>
    /// 使用自定义 HttpClient 下载文件
    /// </summary>
    public static async Task DownloadFileAsync(HttpClient client, string url, string fileFullPath, IProgress<int>? progress = null)
    {
        using var response = await client.GetAsync(url, HttpCompletionOption.ResponseHeadersRead);
        response.EnsureSuccessStatusCode();

        var totalBytes = response.Content.Headers.ContentLength ?? -1;
        await using var contentStream = await response.Content.ReadAsStreamAsync();
        await using var fileStream = new FileStream(fileFullPath, FileMode.Create, FileAccess.Write, FileShare.None, 4096, true);

        var buffer = new byte[8192];
        long totalRead = 0;
        int bytesRead;
        var lastReportedPercent = -1;

        while ((bytesRead = await contentStream.ReadAsync(buffer)) > 0)
        {
            await fileStream.WriteAsync(buffer.AsMemory(0, bytesRead));
            totalRead += bytesRead;

            if (progress != null && totalBytes > 0)
            {
                var percent = (int)((double)totalRead / totalBytes * 100);
                if (percent != lastReportedPercent)
                {
                    lastReportedPercent = percent;
                    progress.Report(percent);
                }
            }
        }

        progress?.Report(100);
    }
}
