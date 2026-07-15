using System.Collections.Immutable;
using System.Diagnostics;
using System.Text;
using Hatch.Models;
using Hatch.Properties;
using Hatch.Utils;

namespace Hatch.Services;

public class Updater
{
    private string UpdateFile { get; }

    private string InstallDirectory { get; }

    private readonly string _tempDirectory;
    private static readonly string[] KeepDirectories = { "data", "mode\\Custom", "logging" };
    private static readonly string[] KeepFiles = { Constants.DisableModeDirectoryFileName };

    internal Updater(string updateFile, string installDirectory)
    {
        UpdateFile = updateFile;
        InstallDirectory = installDirectory;
        _tempDirectory = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName());

        Directory.CreateDirectory(_tempDirectory);
    }

    #region Apply Update

    internal async Task ApplyUpdateAsync(string expectedSha256)
    {
        // 强制校验 SHA256
        if (string.IsNullOrEmpty(expectedSha256))
            throw new ArgumentException("SHA256 checksum is required for update verification", nameof(expectedSha256));

        Log.Information("Verifying update file SHA256...");
        var actualSha256 = await Utils.Utils.Sha256CheckSumAsync(UpdateFile);

        if (string.IsNullOrEmpty(actualSha256))
            throw new MessageException(i18N.Translate("Failed to compute SHA256 checksum"));

        if (!actualSha256.Equals(expectedSha256, StringComparison.OrdinalIgnoreCase))
        {
            Log.Error("SHA256 mismatch: expected {Expected}, got {Actual}", expectedSha256, actualSha256);
            throw new MessageException(i18N.Translate("Update file integrity check failed (SHA256 mismatch)"));
        }

        Log.Information("SHA256 verification passed");

        var extractPath = Path.Combine(_tempDirectory, "extract");

        int exitCode;
        if ((exitCode = Extract(extractPath)) != 0)
            throw new MessageException(i18N.Translate($"7za unexpectedly exited. ({exitCode})"));

        // update archive file must have a top-level directory "Hatch"
        var updateDirectory = Path.Combine(extractPath, "Hatch");
        if (!Directory.Exists(updateDirectory))
            throw new MessageException(i18N.Translate("Update file top-level directory not exist"));

        // {_tempDirectory}/extract/Hatch/Hatch.exe
        var updateMainProgramFilePath = Path.Combine(updateDirectory, "Hatch.exe");
        if (!File.Exists(updateMainProgramFilePath))
            throw new MessageException(i18N.Translate($"Update file main program not exist"));

        MarkFilesOld();

        // Move {tempDirectory}\extract\Hatch to the install folder.
        MoveFilesOver(updateDirectory, InstallDirectory);
    }

    [Obsolete("Use ApplyUpdateAsync with SHA256 parameter instead")]
    internal void ApplyUpdate(string expectedSha256)
    {
        ApplyUpdateAsync(expectedSha256).GetAwaiter().GetResult();
    }

    private void MarkFilesOld()
    {
        var keepDirFullPath = KeepDirectories.Select(d => Path.Combine(InstallDirectory, d)).ToImmutableList();

        foreach (var file in Directory.GetFiles(InstallDirectory, "*", SearchOption.AllDirectories))
        {
            if (keepDirFullPath.Any(p => file.StartsWith(p)))
                continue;

            if (KeepFiles.Contains(Path.GetFileName(file)))
                continue;

            try
            {
                File.Move(file, file + ".old");
            }
            catch (Exception e)
            {
                Log.Error(e, "failed to rename file \"{File}\"", file);
                throw;
            }
        }
    }

    private int Extract(string destDirName)
    {
        // release 7za.exe to {tempDirectory}\7za.exe
        var temp7za = Path.Combine(_tempDirectory, "7za.exe");

        if (!File.Exists(temp7za))
            File.WriteAllBytes(temp7za, Resources._7za);

        var argument = new StringBuilder($" x \"{UpdateFile}\" -o\"{destDirName}\" -y");
        var process = Process.Start(new ProcessStartInfo(temp7za, argument.ToString())
        {
            UseShellExecute = false
        })!;

        process.WaitForExit();
        return process.ExitCode;
    }

    private static void MoveFilesOver(string source, string target)
    {
        foreach (string directory in Directory.GetDirectories(source))
        {
            string dirName = Path.GetFileName(directory);

            if (!Directory.Exists(Path.Combine(target, dirName)))
                Directory.CreateDirectory(Path.Combine(target, dirName));

            MoveFilesOver(directory, Path.Combine(target, dirName));
        }

        foreach (string file in Directory.GetFiles(source))
        {
            var destFile = Path.Combine(target, Path.GetFileName(file));
            File.Delete(destFile);
            File.Move(file, destFile);
        }
    }

    #endregion

    #region Clean files marked as old when start

    public static void CleanOld(string targetPath)
    {
        foreach (var f in Directory.GetFiles(targetPath, "*.old", SearchOption.AllDirectories))
        {
            try
            {
                File.Delete(f);
            }
            catch (Exception e)
            {
                Log.Warning(e, "Failed to delete old file: {File}", f);
            }
        }
    }

    #endregion
}
