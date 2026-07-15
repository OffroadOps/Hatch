using System.Security.Cryptography;
using System.Text;

namespace Hatch.Utils;

/// <summary>
///     使用 DPAPI 保护敏感凭据（密码等），加密数据绑定到当前用户。
/// </summary>
public static class CredentialProtection
{
    private const string EncryptedPrefix = "DPAPI:";

    /// <summary>
    ///     加密字符串，返回带前缀的 Base64 密文。
    /// </summary>
    public static string Protect(string? plainText)
    {
        if (string.IsNullOrEmpty(plainText))
            return string.Empty;

        try
        {
            var plainBytes = Encoding.UTF8.GetBytes(plainText);
            var encryptedBytes = ProtectedData.Protect(plainBytes, null, DataProtectionScope.CurrentUser);
            return EncryptedPrefix + Convert.ToBase64String(encryptedBytes);
        }
        catch (Exception e)
        {
            Log.Warning(e, "DPAPI Protect failed, storing as-is");
            return plainText;
        }
    }

    /// <summary>
    ///     解密字符串，自动识别是否为 DPAPI 加密格式。
    /// </summary>
    public static string Unprotect(string? protectedText)
    {
        if (string.IsNullOrEmpty(protectedText))
            return string.Empty;

        if (!protectedText.StartsWith(EncryptedPrefix))
            return protectedText; // 未加密的明文（兼容旧配置）。

        try
        {
            var encryptedBytes = Convert.FromBase64String(protectedText[EncryptedPrefix.Length..]);
            var plainBytes = ProtectedData.Unprotect(encryptedBytes, null, DataProtectionScope.CurrentUser);
            return Encoding.UTF8.GetString(plainBytes);
        }
        catch (Exception e)
        {
            Log.Warning(e, "DPAPI Unprotect failed");
            return string.Empty;
        }
    }

    /// <summary>
    ///     判断字符串是否已经是 DPAPI 加密格式。
    /// </summary>
    public static bool IsProtected(string? text)
    {
        return !string.IsNullOrEmpty(text) && text.StartsWith(EncryptedPrefix);
    }
}
