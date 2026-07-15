using System.Globalization;
using System.Reflection;
using System.Runtime.Versioning;
using Windows.Win32;
using Windows.Win32.Foundation;
using Microsoft.VisualStudio.Threading;
using Hatch.Controllers;
using Hatch.Enums;
using Hatch.Forms;
using Hatch.Models;
using Hatch.Services;
using Hatch.Utils;
using Serilog.Events;
using SingleInstance;
#if RELEASE
using Windows.Win32.UI.WindowsAndMessaging;
#endif

namespace Hatch;

public static class Program
{
    public static readonly ISingleInstanceService SingleInstance = new SingleInstanceService(
        $"Global\\{nameof(Hatch)}_{Environment.UserName.GetHashCode():X8}");

    internal static HWND ConsoleHwnd { get; private set; }

#pragma warning disable VSTHRD002
    // VSTHRD002: Avoid problematic synchronous waits
    // Main never re-called, so we can ignore this

    [STAThread]
    public static void Main(string[] args)
    {
        try
        {
        // handle arguments
        if (args.Contains(Constants.Parameter.ForceUpdate))
            Flags.AlwaysShowNewVersionFound = true;

        // set working directory
        Directory.SetCurrentDirectory(Global.HatchDir);

        // append .\bin to PATH (prepend to avoid DLL hijacking from other PATH entries)
        var binPath = Path.GetFullPath(Path.Combine(Global.HatchDir, "bin"));
        Environment.SetEnvironmentVariable("PATH", $"{binPath};{Environment.GetEnvironmentVariable("PATH")}");

#if !DEBUG
        // check if .\bin directory exists
        var binDir = Path.Combine(Global.HatchDir, "bin");
        if (!Directory.Exists(binDir) || !Directory.EnumerateFileSystemEntries(binDir).Any())
        {
            i18N.Load("System");
            MessageBoxX.Show(i18N.Translate("Please extract all files then run the program!"));
            Environment.Exit(2);
        }
#endif
        // clean up old files
        Updater.CleanOld(Global.HatchDir);

        // pre-create directories
        var directories = new[] { "mode\\Custom", "data", "i18n", "logging" };
        foreach (var item in directories)
            if (!Directory.Exists(item))
                Directory.CreateDirectory(item);

        // clean up old logs
        if (Directory.Exists("logging"))
        {
            try
            {
                var directory = new DirectoryInfo("logging");

                foreach (var file in directory.GetFiles())
                {
                    try { file.Delete(); } catch { /* file may be locked */ }
                }

                foreach (var dir in directory.GetDirectories())
                {
                    try { dir.Delete(true); } catch { /* dir may be locked */ }
                }
            }
            catch
            {
                // ignored - don't let log cleanup prevent startup
            }
        }

        InitConsole();

        CreateLogger();
        Log.Information("Hatch Startup - Entry Point");

        // load configuration
        Log.Information("Loading configuration...");
        try
        {
            var loadTask = Task.Run(async () => await Configuration.LoadAsync());
            if (!loadTask.Wait(TimeSpan.FromSeconds(10)))
            {
                Log.Warning("Configuration loading timed out, using default settings");
                Global.Settings = new Setting();
            }
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Configuration loading failed, using default settings");
            Global.Settings = new Setting();
        }
        Log.Information("Configuration loaded");

        // check core availability
        CheckCoreAvailability();

        // check if the program is already running
        if (!SingleInstance.TryStartSingleInstance())
        {
            try
            {
                var sendTask = Task.Run(async () =>
                    await SingleInstance.SendMessageToFirstInstanceAsync(Constants.Parameter.Show));
                sendTask.Wait(TimeSpan.FromSeconds(5));
            }
            catch (Exception ex)
            {
                Log.Warning(ex, "Failed to send message to first instance");
            }
            Environment.Exit(0);
            return;
        }

        SingleInstance.Received.Subscribe(SingleInstance_ArgumentsReceived);





        // load i18n
        i18N.Load(Global.Settings.Language);

        // log environment information
        LogEnvironmentAsync().ContinueWith(t =>
        {
            if (t.IsFaulted && t.Exception != null)
                Log.Error(t.Exception, "Failed to log environment information");
        });
        CheckClr();
        CheckOS();

        // handle exceptions
        Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
        Application.ThreadException += Application_OnException;
        Application.ApplicationExit += Application_OnExit;

        Application.SetHighDpiMode(HighDpiMode.DpiUnawareGdiScaled);
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);

        Application.Run(Global.MainForm);
        }
        catch (Exception ex)
        {
            try
            {
                Log.Fatal(ex, "Fatal error during startup");
                Log.CloseAndFlush();
            }
            catch { /* ignored */ }

            MessageBox.Show($"Hatch failed to start:\n\n{ex.Message}\n\nPlease check logging/application.log for details.",
                "Hatch Startup Error", MessageBoxButtons.OK, MessageBoxIcon.Error);
            Environment.Exit(-1);
        }
    }

#pragma warning restore VSTHRD002

    private static async Task LogEnvironmentAsync()
    {
        Log.Information("Hatch Version: {Version}", $"{UpdateChecker.Owner}/{UpdateChecker.Repo}@{UpdateChecker.Version}");
        Log.Information("OS: {OSVersion}", Environment.OSVersion);
        Log.Information("SHA256: {Hash}", $"{await Utils.Utils.Sha256CheckSumAsync(Global.HatchExecutable)}");
        Log.Information("System Language: {Language}", CultureInfo.CurrentCulture.Name);

#if RELEASE
        if (Log.IsEnabled(LogEventLevel.Debug))
        {
            // TODO log level setting
            Task.Run(() => Log.Debug("Third-party Drivers:\n{Drivers}", string.Join(Constants.EOF, SystemInfo.SystemDrivers(false))))
                .ContinueWith(t =>
                {
                    if (t.IsFaulted && t.Exception != null)
                        Log.Error(t.Exception, "Failed to log drivers");
                });
            Task.Run(() => Log.Debug("Running Processes: \n{Processes}", string.Join(Constants.EOF, SystemInfo.Processes(false))))
                .ContinueWith(t =>
                {
                    if (t.IsFaulted && t.Exception != null)
                        Log.Error(t.Exception, "Failed to log processes");
                });
        }
#endif
    }

    private static void CheckClr()
    {
        var framework = Assembly.GetExecutingAssembly().GetCustomAttribute<TargetFrameworkAttribute>()?.FrameworkName;
        if (framework == null)
        {
            Log.Warning("TargetFrameworkAttribute null");
            return;
        }

        var frameworkName = new FrameworkName(framework);

        if (frameworkName.Version.Major != Environment.Version.Major)
        {
            Log.Information("CLR: {Version}", Environment.Version);
            Flags.NoSupport = true;
            if (!Global.Settings.NoSupportDialog)
                MessageBoxX.Show(
                    i18N.TranslateFormat("{0} won't get developers' support, Please do not report any issues or seek help from developers.",
                        "CLR " + Environment.Version),
                    LogLevel.WARNING);
        }
    }

    private static void CheckOS()
    {
        if (Environment.OSVersion.Version.Build < 17763)
        {
            Flags.NoSupport = true;
            if (!Global.Settings.NoSupportDialog)
                MessageBoxX.Show(
                    i18N.TranslateFormat("{0} won't get developers' support, Please do not report any issues or seek help from developers.",
                        Environment.OSVersion),
                    LogLevel.WARNING);
        }
    }

    private static void CheckCoreAvailability()
    {
        var missingCores = CoreSelector.GetMissingCores();

        if (!missingCores.Any())
        {
            // All cores are available
            Log.Information("Core availability check: All cores available");
            return;
        }

        // Log missing cores
        foreach (var (coreType, description) in missingCores)
        {
            var coreName = CoreSelector.GetCoreFullName(coreType);
            var coreExe = CoreSelector.GetCoreExecutableName(coreType);
            Log.Warning("Missing core: {Core} (bin\\{Exe}) - {Description}", coreName, coreExe, description);
        }

        // Build warning message
        var message = i18N.Translate("Detected missing cores:") + "\n\n";

        foreach (var (coreType, description) in missingCores)
        {
            var coreName = CoreSelector.GetCoreFullName(coreType);
            var coreExe = CoreSelector.GetCoreExecutableName(coreType);
            message += $"? {coreName} (bin\\{coreExe})\n  {description}\n\n";
        }

        message += i18N.Translate("Some protocols may not work.") + "\n";
        message += i18N.Translate("Download the complete version?");

        var result = MessageBoxX.Show(message, LogLevel.WARNING, i18N.Translate("Core Check"), true);

        if (result == DialogResult.OK)
        {
            // Open download page
            try
            {
                Utils.Utils.Open("https://github.com/OffroadOps/Hatch/releases");
            }
            catch (Exception e)
            {
                Log.Warning(e, "Failed to open download page");
            }
        }
    }

    private static void InitConsole()
    {
#if RELEASE
        // 在 Release 模式下，先创建控制台但不显示
        PInvoke.AllocConsole();
        ConsoleHwnd = PInvoke.GetConsoleWindow();
        // 立即隐藏控制台窗口，避免闪烁
        PInvoke.ShowWindow(ConsoleHwnd, SHOW_WINDOW_CMD.SW_HIDE);
#else
        // Debug 模式下正常显示控制台
        PInvoke.AllocConsole();
        ConsoleHwnd = PInvoke.GetConsoleWindow();
#endif
    }

    public static void CreateLogger()
    {
        Log.Logger = new LoggerConfiguration()
#if DEBUG
            .MinimumLevel.Verbose()
#else
            .MinimumLevel.Debug()
#endif
            .WriteTo.Async(c => c.File(Path.Combine(Global.HatchDir, Constants.LogFile),
                outputTemplate: Constants.OutputTemplate,
                rollOnFileSizeLimit: false))
            .WriteTo.Console(outputTemplate: Constants.OutputTemplate)
            .MinimumLevel.Override(@"Microsoft", LogEventLevel.Information)
            .Enrich.FromLogContext()
            .CreateLogger();
    }

    private static void Application_OnException(object sender, ThreadExceptionEventArgs e)
    {
        Log.Error(e.Exception, "Unhandled error");
    }

    private static void Application_OnExit(object? sender, EventArgs eventArgs)
    {
        Log.CloseAndFlush();
    }

    private static void SingleInstance_ArgumentsReceived((string, Action<string>) receive)
    {
        var (arg, endFunc) = receive;
        if (arg == Constants.Parameter.Show)
        {
            Utils.Utils.ActivateVisibleWindows();
        }

        endFunc(string.Empty);
    }
}
