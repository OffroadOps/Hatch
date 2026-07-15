using System.Diagnostics;
using System.Text;
using System.Text.Json;

namespace Hatch.Utils;

public sealed class WindowsStoreAppInfo
{
    public string PackageName { get; set; } = string.Empty;

    public string PackageFullName { get; set; } = string.Empty;

    public string PackageFamilyName { get; set; } = string.Empty;

    public string InstallLocation { get; set; } = string.Empty;

    public string AppId { get; set; } = string.Empty;

    public string DisplayName { get; set; } = string.Empty;

    public string Executable { get; set; } = string.Empty;

    public string Title
    {
        get
        {
            var name = DisplayName.ValueOrDefault(PackageName);
            if (name?.StartsWith("ms-resource:", StringComparison.OrdinalIgnoreCase) == true)
                name = PackageName.ValueOrDefault(PackageFullName);

            var executable = Executable.ValueOrDefault();
            return executable == null ? $"{name} ({PackageName})" : $"{name} - {Path.GetFileName(executable)} ({PackageName})";
        }
    }

    public string ToProcessRule()
    {
        var target = GetExecutableFullName();
        if (target == null)
        {
            var location = InstallLocation.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;
            return "^" + location.ToRegexString();
        }

        return "^" + target.ToRegexString();
    }

    private string? GetExecutableFullName()
    {
        var executable = Executable.ValueOrDefault();
        var installLocation = InstallLocation.ValueOrDefault();
        if (executable == null || installLocation == null)
            return null;

        return Path.IsPathRooted(executable) ? executable : Path.Combine(installLocation, executable);
    }

    public override string ToString() => Title;
}

public static class WindowsStoreAppHelper
{
    private static readonly JsonSerializerOptions JsonSerializerOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    public static async Task<IReadOnlyList<WindowsStoreAppInfo>> GetInstalledAppsAsync()
    {
        var script = """
            $ErrorActionPreference = 'SilentlyContinue'
            Get-AppxPackage | ForEach-Object {
                $pkg = $_
                if ([string]::IsNullOrWhiteSpace($pkg.InstallLocation)) { return }

                $manifestPath = Join-Path -Path $pkg.InstallLocation -ChildPath 'AppxManifest.xml'
                if (!(Test-Path -LiteralPath $manifestPath)) { return }

                try {
                    [xml]$manifest = Get-Content -LiteralPath $manifestPath -Raw
                }
                catch {
                    return
                }

                foreach ($app in $manifest.Package.Applications.Application) {
                    $displayName = $app.VisualElements.DisplayName
                    if ([string]::IsNullOrWhiteSpace($displayName)) { $displayName = $pkg.Name }

                    [pscustomobject]@{
                        PackageName       = $pkg.Name
                        PackageFullName   = $pkg.PackageFullName
                        PackageFamilyName = $pkg.PackageFamilyName
                        InstallLocation   = $pkg.InstallLocation
                        AppId             = $app.Id
                        DisplayName       = $displayName
                        Executable        = $app.Executable
                    }
                }
            } | Sort-Object DisplayName, PackageName, Executable -Unique | ConvertTo-Json -Depth 4
            """;

        var encodedCommand = Convert.ToBase64String(Encoding.Unicode.GetBytes(script));
        using var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = "powershell.exe",
                Arguments = $"-NoProfile -ExecutionPolicy Bypass -EncodedCommand {encodedCommand}",
                CreateNoWindow = true,
                RedirectStandardError = true,
                RedirectStandardOutput = true,
                UseShellExecute = false
            }
        };

        process.Start();
        var stdoutTask = process.StandardOutput.ReadToEndAsync();
        var stderrTask = process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();

        var stdout = await stdoutTask;
        var stderr = await stderrTask;
        if (process.ExitCode != 0)
            throw new InvalidOperationException(stderr.ValueOrDefault("Failed to enumerate Windows Store apps"));

        if (stdout.IsNullOrWhiteSpace())
            return Array.Empty<WindowsStoreAppInfo>();

        using var document = JsonDocument.Parse(stdout);
        var apps = document.RootElement.ValueKind switch
        {
            JsonValueKind.Array => JsonSerializer.Deserialize<List<WindowsStoreAppInfo>>(stdout, JsonSerializerOptions),
            JsonValueKind.Object => new List<WindowsStoreAppInfo>
            {
                JsonSerializer.Deserialize<WindowsStoreAppInfo>(stdout, JsonSerializerOptions)!
            },
            _ => new List<WindowsStoreAppInfo>()
        };

        return (apps ?? new List<WindowsStoreAppInfo>())
            .Where(app => !app.InstallLocation.IsNullOrWhiteSpace())
            .GroupBy(app => app.ToProcessRule(), StringComparer.OrdinalIgnoreCase)
            .Select(group => group.First())
            .OrderBy(app => app.Title, StringComparer.CurrentCultureIgnoreCase)
            .ToList();
    }
}