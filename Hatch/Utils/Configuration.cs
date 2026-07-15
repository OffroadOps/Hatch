using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.VisualStudio.Threading;
using Hatch.JsonConverter;
using Hatch.Models;
using Hatch.Servers;

namespace Hatch.Utils;

public static class Configuration
{
    /// <summary>
    ///     数据目录
    /// </summary>
    public static string DataDirectoryFullName => Path.Combine(Global.HatchDir, "data");

    public static string FileFullName => Path.Combine(DataDirectoryFullName, FileName);

    private static string BackupFileFullName => Path.Combine(DataDirectoryFullName, BackupFileName);

    private const string FileName = "settings.json";

    private const string BackupFileName = "settings.json.bak";

    private static readonly AsyncReaderWriterLock _lock = new(null);

    private static readonly JsonSerializerOptions JsonSerializerOptions = Global.NewCustomJsonSerializerOptions();

    static Configuration()
    {
        JsonSerializerOptions.Converters.Add(new ServerConverterWithTypeDiscriminator());
        JsonSerializerOptions.Converters.Add(new JsonStringEnumConverter());
    }

    public static async Task LoadAsync()
    {
        try
        {
            if (!File.Exists(FileFullName))
            {
                await SaveAsync();
                return;
            }

            await using var _ = await _lock.ReadLockAsync();

            if (await LoadCoreAsync(FileFullName))
                return;

            Log.Information("Load backup configuration \"{FileName}\"", BackupFileFullName);
            if (await LoadCoreAsync(BackupFileFullName))
                return;

            // 主配置和备份都加载失败，使用默认配置
            Log.Warning("Both configuration files failed to load, using default settings");
            Global.Settings = new Setting();

            // 尝试备份损坏的配置文件
            try
            {
                if (File.Exists(FileFullName))
                {
                    var corruptBackup = FileFullName + ".corrupt." + DateTime.Now.ToString("yyyyMMddHHmmss");
                    File.Move(FileFullName, corruptBackup);
                    Log.Information("Moved corrupt config to {Path}", corruptBackup);
                }
            }
            catch (Exception moveEx)
            {
                Log.Warning(moveEx, "Failed to backup corrupt configuration file");
            }
        }
        catch (Exception e)
        {
            Log.Error(e, "Load configuration failed, using default settings");
            Global.Settings = new Setting();
        }
    }

    private static async ValueTask<bool> LoadCoreAsync(string filename)
    {
        try
        {
            Setting settings;

            await using (var fs = new FileStream(filename, FileMode.Open, FileAccess.Read, FileShare.Read, 4096, true))
            {
                settings = (await JsonSerializer.DeserializeAsync<Setting>(fs, JsonSerializerOptions))!;
            }

            CheckSetting(settings);
            Global.Settings = settings;
            return true;
        }
        catch (Exception e)
        {
            Log.Error(e, "Load configuration file \"{FileName}\" error ", filename);
            return false;
        }
    }

    private static void CheckSetting(Setting settings)
    {
        settings.Profiles.RemoveAll(p => p.ServerRemark == string.Empty || p.ModeRemark == string.Empty);

        if (settings.Profiles.Any(p => settings.Profiles.Any(p1 => p1 != p && p1.Index == p.Index)))
            for (var i = 0; i < settings.Profiles.Count; i++)
                settings.Profiles[i].Index = i;

        settings.AioDNS.ChinaDNS = DnsUtils.AppendPort(settings.AioDNS.ChinaDNS);
        settings.AioDNS.OtherDNS = DnsUtils.AppendPort(settings.AioDNS.OtherDNS);

        // 解密服务器凭据
        DecryptCredentials(settings);
    }

    /// <summary>
    ///     解密配置中的服务器凭据（加载时调用）
    /// </summary>
    private static void DecryptCredentials(Setting settings)
    {
        foreach (var server in settings.Server)
        {
            if (server is Servers.Socks5Server socks5)
            {
                socks5.Password = CredentialProtection.Unprotect(socks5.Password);
                socks5.Username = CredentialProtection.Unprotect(socks5.Username);
            }
            else if (server is Servers.ShadowsocksServer ss)
            {
                ss.Password = CredentialProtection.Unprotect(ss.Password);
            }
            else if (server is Servers.ShadowsocksRServer ssr)
            {
                ssr.Password = CredentialProtection.Unprotect(ssr.Password);
            }
            else if (server is Servers.Hysteria2.Hysteria2Server hy2)
            {
                hy2.Password = CredentialProtection.Unprotect(hy2.Password);
            }
            else if (server is Servers.TrojanServer trojan)
            {
                trojan.Password = CredentialProtection.Unprotect(trojan.Password);
            }
            else if (server is Servers.VMessServer vmess)
            {
                vmess.UserID = CredentialProtection.Unprotect(vmess.UserID);
                vmess.QUICSecret = CredentialProtection.Unprotect(vmess.QUICSecret);
            }
        }
    }

    /// <summary>
    ///     加密配置中的服务器凭据（保存时调用，操作副本）
    /// </summary>
    private static Setting EncryptCredentials(Setting settings)
    {
        var copy = settings.ShallowCopy();
        copy.Server = new List<Server>(settings.Server.Count);

        foreach (var server in settings.Server)
        {
            var cloned = (Server)server.Clone();

            if (cloned is Servers.Socks5Server socks5)
            {
                socks5.Password = CredentialProtection.Protect(socks5.Password);
                socks5.Username = CredentialProtection.Protect(socks5.Username);
            }
            else if (cloned is Servers.ShadowsocksServer ss)
            {
                ss.Password = CredentialProtection.Protect(ss.Password);
            }
            else if (cloned is Servers.ShadowsocksRServer ssr)
            {
                ssr.Password = CredentialProtection.Protect(ssr.Password);
            }
            else if (cloned is Servers.Hysteria2.Hysteria2Server hy2)
            {
                hy2.Password = CredentialProtection.Protect(hy2.Password);
            }
            else if (cloned is Servers.TrojanServer trojan)
            {
                trojan.Password = CredentialProtection.Protect(trojan.Password);
            }
            else if (cloned is Servers.VMessServer vmess)
            {
                vmess.UserID = CredentialProtection.Protect(vmess.UserID);
                vmess.QUICSecret = CredentialProtection.Protect(vmess.QUICSecret);
            }

            copy.Server.Add(cloned);
        }

        return copy;
    }

    /// <summary>
    ///     保存配置
    /// </summary>
    public static async Task SaveAsync()
    {
        if (_lock.IsWriteLockHeld)
            return;

        try
        {
            await using var _ = await _lock.WriteLockAsync();
            Log.Verbose("Save Configuration");

            if (!Directory.Exists(DataDirectoryFullName))
                Directory.CreateDirectory(DataDirectoryFullName);

            // 保存时加密凭据（操作副本，不影响内存中的明文）
            var settingsToSave = EncryptCredentials(Global.Settings);

            var tempFile = FileFullName + ".tmp";
            await using (var fileStream = new FileStream(tempFile, FileMode.Create, FileAccess.Write, FileShare.None, 4096, true))
            {
                await JsonSerializer.SerializeAsync(fileStream, settingsToSave, JsonSerializerOptions);
            }

            await EnsureConfigFileExistsAsync();

            File.Replace(tempFile, FileFullName, BackupFileFullName);
        }
        catch (Exception e)
        {
            Log.Error(e, "Save Configuration error");
        }
    }

    private static async ValueTask EnsureConfigFileExistsAsync()
    {
        if (!File.Exists(FileFullName))
        {
            await using var fs = new FileStream(FileFullName, FileMode.Create, FileAccess.ReadWrite, FileShare.None, 4096, true);
        }
    }
}