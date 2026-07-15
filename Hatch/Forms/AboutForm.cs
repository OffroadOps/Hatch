using System.Diagnostics;
using Serilog;

namespace Hatch.Forms;

public partial class AboutForm : Form
{
    public AboutForm()
    {
        InitializeComponent();
        Hatch.Utils.i18N.TranslateForm(this);
        LoadVersionInfo();
    }

    private void LoadVersionInfo()
    {
        // 软件版本
        AppVersionLabel.Text = $"Hatch v{Constants.Version}";

        // Xray 版本
        var xrayPath = Path.Combine("bin", "xray.exe");
        if (File.Exists(xrayPath))
        {
            try
            {
                var xrayVersion = GetCoreVersion(xrayPath, "--version");
                XrayVersionLabel.Text = $"Xray-core: {xrayVersion}";
                XrayVersionLabel.ForeColor = Color.Green;
            }
            catch
            {
                XrayVersionLabel.Text = Utils.i18N.Translate("Xray-core: Installed");
                XrayVersionLabel.ForeColor = Color.Green;
            }
        }
        else
        {
            XrayVersionLabel.Text = Utils.i18N.Translate("Xray-core: Not installed");
            XrayVersionLabel.ForeColor = Color.Red;
        }

        // sing-box 版本
        var singboxPath = Path.Combine("bin", "sing-box.exe");
        if (File.Exists(singboxPath))
        {
            try
            {
                var singboxVersion = GetCoreVersion(singboxPath, "version");
                SingBoxVersionLabel.Text = $"sing-box: {singboxVersion}";
                SingBoxVersionLabel.ForeColor = Color.Green;
            }
            catch
            {
                SingBoxVersionLabel.Text = Utils.i18N.Translate("sing-box: Installed");
                SingBoxVersionLabel.ForeColor = Color.Green;
            }
        }
        else
        {
            SingBoxVersionLabel.Text = Utils.i18N.Translate("sing-box: Not installed");
            SingBoxVersionLabel.ForeColor = Color.Red;
        }
    }

    private string GetCoreVersion(string exePath, string args)
    {
        try
        {
            var process = new Process
            {
                StartInfo = new ProcessStartInfo
                {
                    FileName = exePath,
                    Arguments = args,
                    UseShellExecute = false,
                    RedirectStandardOutput = true,
                    CreateNoWindow = true
                }
            };

            process.Start();
            var output = process.StandardOutput.ReadToEnd();
            process.WaitForExit(3000);

            // 提取版本号（第一行通常包含版本信息）
            var lines = output.Split('\n');
            if (lines.Length > 0)
            {
                var firstLine = lines[0].Trim();
                // 尝试提取版本号
                var match = System.Text.RegularExpressions.Regex.Match(firstLine, @"v?\d+\.\d+\.\d+");
                if (match.Success)
                    return match.Value;
                return firstLine.Length > 50 ? firstLine.Substring(0, 50) + "..." : firstLine;
            }

            return "Unknown";
        }
        catch
        {
            return "Unknown";
        }
    }

    private void CheckUpdateButton_Click(object sender, EventArgs e)
    {
        CheckUpdateButton.Enabled = false;
        CheckUpdateButton.Text = Utils.i18N.Translate("Checking...");

        Task.Run(async () =>
        {
            try
            {
                // 复用 UpdateChecker 统一逻辑
                var succeeded = await Controllers.UpdateChecker.CheckAsync(Global.Settings.CheckBetaUpdate);
                if (!succeeded)
                    throw new InvalidOperationException(Utils.i18N.Translate("Check update failed"));

                this.Invoke(() =>
                {
                    var latestVersion = Controllers.UpdateChecker.LatestVersionNumber;
                    var currentVersion = Controllers.UpdateChecker.Version;

                    if (Models.GitHubRelease.VersionUtil.CompareVersion(latestVersion, currentVersion) > 0)
                    {
                        var result = MessageBox.Show(
                            Utils.i18N.TranslateFormat("New version available: {0}\n\nCurrent version: {1}\n\nWould you like to download it?", latestVersion, currentVersion),
                            Utils.i18N.Translate("Update Available"),
                            MessageBoxButtons.YesNo,
                            MessageBoxIcon.Information);

                        if (result == DialogResult.Yes)
                        {
                            Process.Start(new ProcessStartInfo
                            {
                                FileName = Controllers.UpdateChecker.LatestVersionUrl,
                                UseShellExecute = true
                            });
                        }
                    }
                    else
                    {
                        MessageBox.Show(
                            Utils.i18N.Translate("You are using the latest version!"),
                            Utils.i18N.Translate("No Update"),
                            MessageBoxButtons.OK,
                            MessageBoxIcon.Information);
                    }

                    CheckUpdateButton.Text = Utils.i18N.Translate("Check for Updates");
                    CheckUpdateButton.Enabled = true;
                });
            }
            catch (Exception ex)
            {
                this.Invoke(() =>
                {
                    MessageBox.Show(
                        Utils.i18N.TranslateFormat("Failed to check for updates:\n{0}", ex.Message),
                        Utils.i18N.Translate("Error"),
                        MessageBoxButtons.OK,
                        MessageBoxIcon.Error);

                    CheckUpdateButton.Text = Utils.i18N.Translate("Check for Updates");
                    CheckUpdateButton.Enabled = true;
                });
            }
        });
    }

    private void UpdateCoresButton_Click(object sender, EventArgs e)
    {
        UpdateCoresButton.Enabled = false;
        UpdateCoresButton.Text = Utils.i18N.Translate("Updating...");

        Task.Run(async () =>
        {
            try
            {
                var results = await UpdateCoresAsync();

                this.Invoke(() =>
                {
                    var message = Utils.i18N.TranslateFormat("Core update results:\n\n{0}", string.Join("\n", results));
                    MessageBox.Show(message, Utils.i18N.Translate("Update Complete"), MessageBoxButtons.OK, MessageBoxIcon.Information);

                    LoadVersionInfo(); // 刷新版本信息
                    UpdateCoresButton.Text = Utils.i18N.Translate("Update Cores");
                    UpdateCoresButton.Enabled = true;
                });
            }
            catch (Exception ex)
            {
                this.Invoke(() =>
                {
                    MessageBox.Show(
                        Utils.i18N.TranslateFormat("Failed to update cores:\n{0}", ex.Message),
                        Utils.i18N.Translate("Error"),
                        MessageBoxButtons.OK,
                        MessageBoxIcon.Error);

                    UpdateCoresButton.Text = Utils.i18N.Translate("Update Cores");
                    UpdateCoresButton.Enabled = true;
                });
            }
        });
    }

    private async Task<List<string>> UpdateCoresAsync()
    {
        var results = new List<string>();

        // 更新 Xray
        try
        {
            var xrayVersion = await DownloadXrayAsync();
            results.Add(Utils.i18N.TranslateFormat("? Xray-core updated to {0}", xrayVersion));
        }
        catch (Exception ex)
        {
            results.Add(Utils.i18N.TranslateFormat("? Xray-core update failed: {0}", ex.Message));
        }

        // 更新 sing-box
        try
        {
            var singboxVersion = await DownloadSingBoxAsync();
            results.Add(Utils.i18N.TranslateFormat("? sing-box updated to {0}", singboxVersion));
        }
        catch (Exception ex)
        {
            results.Add(Utils.i18N.TranslateFormat("? sing-box update failed: {0}", ex.Message));
        }

        return results;
    }

    private async Task<string> DownloadXrayAsync()
    {
        using var client = new HttpClient();
        client.DefaultRequestHeaders.Add("User-Agent", "Hatch");

        var response = await client.GetStringAsync("https://api.github.com/repos/XTLS/Xray-core/releases/latest");
        var json = System.Text.Json.JsonDocument.Parse(response);
        var root = json.RootElement;

        var version = root.GetProperty("tag_name").GetString() ?? "unknown";
        var assets = root.GetProperty("assets").EnumerateArray();

        foreach (var asset in assets)
        {
            var name = asset.GetProperty("name").GetString() ?? "";
            if (name.Contains("windows-64") && name.EndsWith(".zip"))
            {
                var downloadUrl = asset.GetProperty("browser_download_url").GetString() ?? "";
                await DownloadAndExtractAsync(downloadUrl, "xray.exe", version);
                return version;
            }
        }

        throw new Exception("Xray Windows release not found");
    }

    private async Task<string> DownloadSingBoxAsync()
    {
        using var client = new HttpClient();
        client.DefaultRequestHeaders.Add("User-Agent", "Hatch");

        var response = await client.GetStringAsync("https://api.github.com/repos/SagerNet/sing-box/releases/latest");
        var json = System.Text.Json.JsonDocument.Parse(response);
        var root = json.RootElement;

        var version = root.GetProperty("tag_name").GetString() ?? "unknown";
        var assets = root.GetProperty("assets").EnumerateArray();

        foreach (var asset in assets)
        {
            var name = asset.GetProperty("name").GetString() ?? "";
            if (name.Contains("windows-amd64") && name.EndsWith(".zip"))
            {
                var downloadUrl = asset.GetProperty("browser_download_url").GetString() ?? "";
                await DownloadAndExtractAsync(downloadUrl, "sing-box.exe", version);
                return version;
            }
        }

        throw new Exception("sing-box Windows release not found");
    }

    private async Task DownloadAndExtractAsync(string url, string exeName, string version)
    {
        using var client = new HttpClient();
        client.Timeout = TimeSpan.FromMinutes(5);
        var zipData = await client.GetByteArrayAsync(url);

        var tempZip = Path.Combine(Path.GetTempPath(), $"{exeName}.zip");
        var tempExtract = Path.Combine(Path.GetTempPath(), $"{exeName}_extract");

        try
        {
            await File.WriteAllBytesAsync(tempZip, zipData);

            // 校验下载完整性（文件大小 + SHA256）
            var fileInfo = new FileInfo(tempZip);
            if (fileInfo.Length != zipData.Length)
                throw new Exception($"Downloaded file size mismatch for {exeName}");

            // 计算 SHA256 校验和
            var sha256 = await Utils.Utils.Sha256CheckSumAsync(tempZip);
            if (string.IsNullOrEmpty(sha256))
                throw new Exception($"Failed to compute SHA256 for {exeName}");

            Log.Information("Downloaded {ExeName} SHA256: {SHA256}", exeName, sha256);

            if (Directory.Exists(tempExtract))
                Directory.Delete(tempExtract, true);

            System.IO.Compression.ZipFile.ExtractToDirectory(tempZip, tempExtract);

            var exeFiles = Directory.GetFiles(tempExtract, exeName, SearchOption.AllDirectories);
            if (exeFiles.Length > 0)
            {
                var binDir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "bin");
                if (!Directory.Exists(binDir))
                    Directory.CreateDirectory(binDir);

                var destPath = Path.Combine(binDir, exeName);
                var backupPath = Path.Combine(binDir, $"{exeName}.{DateTime.UtcNow:yyyyMMddHHmmss}.bak");
                var newExePath = Path.Combine(binDir, $"{exeName}.new");

                if (File.Exists(newExePath))
                    File.Delete(newExePath);

                File.Copy(exeFiles[0], newExePath, true);

                try
                {
                    if (File.Exists(destPath))
                        File.Copy(destPath, backupPath, true);

                    File.Copy(newExePath, destPath, true);
                    await WriteCoreUpdateRecordAsync(exeName, version, url, sha256, backupPath);
                }
                catch
                {
                    if (File.Exists(backupPath))
                        File.Copy(backupPath, destPath, true);

                    throw;
                }
                finally
                {
                    if (File.Exists(newExePath))
                        File.Delete(newExePath);
                }
            }
            else
            {
                throw new Exception($"{exeName} not found in archive");
            }
        }
        finally
        {
            if (File.Exists(tempZip))
                File.Delete(tempZip);
            if (Directory.Exists(tempExtract))
                Directory.Delete(tempExtract, true);
        }
    }

    private static async Task WriteCoreUpdateRecordAsync(string exeName, string version, string url, string sha256, string backupPath)
    {
        var dataDir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "data");
        Directory.CreateDirectory(dataDir);

        var record = new
        {
            exeName,
            version,
            url,
            sha256,
            backupPath = File.Exists(backupPath) ? backupPath : null,
            updatedAtUtc = DateTime.UtcNow
        };

        var line = System.Text.Json.JsonSerializer.Serialize(record) + Environment.NewLine;
        await File.AppendAllTextAsync(Path.Combine(dataDir, "core-updates.jsonl"), line);
    }

    private void GitHubLinkLabel_LinkClicked(object sender, LinkLabelLinkClickedEventArgs e)
    {
        Process.Start(new ProcessStartInfo
        {
            FileName = "https://github.com/OffroadOps/Hatch",
            UseShellExecute = true
        });
    }

    private void NetchLinkLabel_LinkClicked(object sender, LinkLabelLinkClickedEventArgs e)
    {
        Process.Start(new ProcessStartInfo
        {
            FileName = "https://github.com/netchx/netch",
            UseShellExecute = true
        });
    }
}
