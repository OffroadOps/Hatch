using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.Unicode;
using Hatch.Forms;
using Hatch.Models;
using Hatch.Models.Modes;
using WindowsJobAPI;

namespace Hatch;

public static class Global
{
    /// <summary>
    ///     主窗体的静态实例
    /// </summary>
    private static readonly Lazy<MainForm> LazyMainForm = new(() => new MainForm());

    /// <summary>
    ///     用于读取和写入的配置
    /// </summary>
    public static Setting Settings = new();

    public static readonly JobObject Job = new();

    /// <summary>
    ///     用于存储模式
    /// </summary>
    public static readonly List<Mode> Modes = new();

    public static readonly string HatchDir;
    public static readonly string HatchExecutable;

    static Global()
    {
        HatchExecutable = Application.ExecutablePath;

        // 对于单文件发布，使用 exe 所在目录而不是临时解压目录
        var exeDir = Path.GetDirectoryName(Application.ExecutablePath);
        HatchDir = string.IsNullOrEmpty(exeDir) ? Application.StartupPath : exeDir;
    }

    /// <summary>
    ///     主窗体的静态实例
    /// </summary>
    public static MainForm MainForm => LazyMainForm.Value;

    public static JsonSerializerOptions NewCustomJsonSerializerOptions() => new()
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Encoder = JavaScriptEncoder.Create(System.Text.Unicode.UnicodeRanges.All)
    };
}