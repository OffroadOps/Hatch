using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Hatch.Models.GitHubRelease;
using Hatch.Utils;

namespace Hatch.Controllers;

public static class UpdateChecker
{
    public const string Owner = @"OffroadOps";
    public const string Repo = @"Hatch";

    public const string Name = @"Hatch";
    public const string Copyright = @"Copyright © 2026 OffroadOps";

    public const string AssemblyVersion = @"2.0.0";
    private const string Suffix = @"";

    public static readonly string Version = $"{AssemblyVersion}{(string.IsNullOrEmpty(Suffix) ? "" : $"-{Suffix}")}";

    public static Release LatestRelease = null!;

    public static string LatestVersionNumber => LatestRelease.tag_name;

    public static string LatestVersionUrl => LatestRelease.html_url;

    public static event EventHandler? NewVersionFound;

    public static event EventHandler? NewVersionFoundFailed;

    public static event EventHandler? NewVersionNotFound;

    public static async Task<bool> CheckAsync(bool isPreRelease)
    {
        try
        {
            var updater = new GitHubRelease(Owner, Repo);
            var url = updater.AllReleaseUrl;

            var (statusCode, json) = await WebUtil.DownloadStringAsync(url);
            if (statusCode != HttpStatusCode.OK)
                throw new HttpRequestException($"GitHub Releases API returned {(int)statusCode} ({statusCode})");

            var releases = JsonSerializer.Deserialize<List<Release>>(json)
                ?? throw new JsonException("GitHub Releases API returned an empty response");
            LatestRelease = SelectLatestRelease(releases, isPreRelease);
            Log.Information("Github latest release: {Version}", LatestRelease.tag_name);
            if (VersionUtil.CompareVersion(LatestRelease.tag_name, Version) > 0)
            {
                Log.Information("Found newer version");
                NewVersionFound?.Invoke(null, EventArgs.Empty);
            }
            else
            {
                Log.Information("Already the latest version");
                NewVersionNotFound?.Invoke(null, EventArgs.Empty);
            }

            return true;
        }
        catch (Exception e)
        {
            if (e is HttpRequestException)
                Log.Warning(e, "Get releases failed");
            else
                Log.Error(e, "Get releases error");

            NewVersionFoundFailed?.Invoke(null, EventArgs.Empty);
            return false;
        }
    }

    public static (string fileName, string sha256) GetLatestUpdateFileNameAndHash(string? keyword = null)
    {
        var matches = Regex.Matches(
            LatestRelease.body ?? string.Empty,
            @"^\|\s*(?<filename>[^|]+?)\s*\|\s*(?<sha256>[a-fA-F0-9]{64})\s*\|\s*\r?$",
            RegexOptions.Multiline);

        Match match = keyword == null
            ? matches.First()
            : matches.First(m => m.Groups["filename"].Value.Contains(keyword, StringComparison.OrdinalIgnoreCase));

        return (match.Groups["filename"].Value.Trim(), match.Groups["sha256"].Value.ToLowerInvariant());
    }

    public static string GetLatestReleaseContent()
    {
        var sb = new StringBuilder();
        foreach (string l in (LatestRelease.body ?? string.Empty).GetLines(false).SkipWhile(l => l.FirstOrDefault() != '#'))
        {
            if (l.Contains("校验和", StringComparison.OrdinalIgnoreCase) ||
                l.Contains("checksum", StringComparison.OrdinalIgnoreCase))
                break;

            sb.AppendLine(l);
        }

        return sb.ToString();
    }

    public static Release SelectLatestRelease(IEnumerable<Release> releases, bool isPreRelease)
    {
        ArgumentNullException.ThrowIfNull(releases);

        if (!isPreRelease)
            releases = releases.Where(release => !release.prerelease);

        var ordered = releases.OrderByDescending(release => release.tag_name, new VersionUtil.VersionComparer());
        return ordered.FirstOrDefault()
            ?? throw new InvalidOperationException("The repository has no eligible GitHub Releases");
    }
}
